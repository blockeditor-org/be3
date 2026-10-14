# Adding a block

How to add a content type (a block type) end to end, and how its stored form can change later
without migrations. The content lives in `crates/be-block` as a `be-model` document; its UI is a
plugin under `crates/editors` (`guides/plugins/adding_an_editor.md`). For how documents merge,
undo and edit live underneath, see `guides/blocks/stack.md`, "be-model: how documents work".

## Steps

1. Define the model in `crates/be-block/src/my_block.rs`.
2. Write edits as helpers.
3. Declare references, a name and child edits if the type has them.
4. Export it from `crates/be-block/src/lib.rs` and register it in `KINDS`.
5. Add model tests in `crates/be-block/src/tests/`.
6. Run `./scripts/verify`, then add an editor if the block needs a UI.

## 1. Define the model

Describe the content as structs that derive `be_model::Model`. Do not write a merge, a rebase or
an undo: the derive and `Document` provide them.

```rust
use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

use crate::Root;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
#[model(kind = "my_block")]
pub struct MyBlock {
    pub title: String,
    pub items: List<MyItem>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
#[model(kind = "my_block.item")]
pub struct MyItem {
    pub text: String,
    pub done: bool,
}

impl Root for MyBlock {
    // Generate a new, permanent UUID. Never reuse another content type's.
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);
}

pub type MyBlockContent = Document<MyBlock>;
```

- `CONTENT_TYPE` is the block type everywhere: what the server records for the block, what the
  app's registry is keyed by, and the `block_type` a plugin's manifest names. Once a block of it
  is saved, changing it leaves that block with a type nothing opens.
- Give each `Model` struct a kind with `#[model(kind = "...")]`. It is saved with every object
  (without it, the struct's name in snake case is used).
- A block holds what it is, not what an editor makes of it. The image block keeps the file's
  bytes and a header saying what decoding found (or why it failed), which the editor fills in the
  first time it draws. Nothing but the plugin needs a decoder, and no other client has to decode
  the block to know its shape.

### Field kinds

Choose each field's kind by what two people changing it at once should end up with.

| Kind | What it is | Concurrent changes and merge |
|---|---|---|
| register | Any `Serialize + DeserializeOwned + Clone + PartialEq + Default` value, set as a whole. | Setting it on both sides of an offline merge is a conflict. |
| `Count` | A counter. | Concurrent changes add up. |
| `List<T>` | Objects of a `Model` type `T`, each with an `ObjectId`. | Merged by object identity and order; see `guides/blocks/stack.md`. |
| `Map<K, V>` | Values by key, each key its own register. | Two people setting different keys both keep theirs (how a database row holds a cell per schema field). |
| `Latest<T>`, `LatestMap<K, V>` | Last-write-wins. Every write carries a `Stamp` (a time and the writing client). | A write replaces only an older stamp; a merge keeps the later stamp per key. Never conflict, never undo. View state uses them. |
| `Text` | Bytes edited in place. | Live, every byte has a position (`Sequence`, in the `sequence` crate) that edits anchor to, so typing and deleting never rebase. Offline merge is a line diff3. |
| `Grid<T>` | Dense fixed-size cells addressed by coordinates, for pixel data. | Bounds are part of its value: a resize moves the bounds and keeps every cell at its coordinates. `paint` sets cells, `reshape` sets the bounds. |

Notes:

- Give every struct a register holds `#[serde(default)]` (see "Changing the stored form").
- `Latest` writes use `next(time, origin)`, which stamps a write past the one it replaces, so a
  client with a slow clock still overwrites what it last read.
- A `Text` saves only its bytes. Positions and tombstones are session state, and only positions
  travel: the receiver fills the visible bytes in from its own copy, and an undelete carries the
  bytes it brings back. The text block is `Document<TextBlock>`, its body a `Text`
  (`plans/text.md`).
- `Root` names the content type and, optionally, the block's name, its references and its child
  edits.

## 2. Write edits as helpers

The derive gives every field a typed constant (`MyItem::TEXT`, `MyBlock::ITEMS`) that builds the
changes an `Edit` is made of:

| Constant method | For |
|---|---|
| `set` | a register |
| `add` | a `Count` |
| `put` | a key of a `Map` |
| `insert`, `move_into` | a `List` |
| `Change::remove` | any object |

Wrap them in helpers that return `be_model::Edit`, so an editor calls
`content.operate(MyBlock::set_done(id, true))` and never assembles changes itself:

```rust
impl MyBlock {
    pub fn add(text: impl Into<String>) -> (ObjectId, Edit) {
        let item = MyItem { text: text.into(), done: false };
        let (id, change) = Self::ITEMS.insert(ObjectId::ROOT, Anchor::End, &item);
        (id, change.into())
    }

    pub fn set_done(item: ObjectId, done: bool) -> Edit {
        MyItem::DONE.set(item, &done).into()
    }

    pub fn remove(item: ObjectId) -> Edit {
        Change::remove(item).into()
    }

    pub fn clear_done(&self) -> Edit {
        self.items
            .iter()
            .filter(|item| item.done)
            .map(|item| Change::remove(item.id))
            .collect()
    }
}
```

- An edit names an object, never a position: two people removing what they each saw as the
  second item remove the same one, and an insert at the head renumbers nothing.
- A helper that creates an object returns its id, so the caller can address it at once. Editors
  key their rows on those ids, which lets a row survive an edit to its neighbour
  (`guides/beui/reactive.md`).
- A helper that needs the current content to decide what to write (`clear_done`,
  `Counter::reset`) takes `&self`; one that does not is an associated function.
- Write only what changed: `Calendar::update` writes only the fields that differ.
- Examples: `crates/be-block/src/counter.rs` and `checklist.rs` are the smallest complete types;
  `calendar.rs` shows the changed-fields update.

## 3. References, name and children

### References

If the content holds the ids of other blocks, store each as a `be_model::BlockRef`, or mark a
`Uuid` field with `#[serde(with = "be_model::references::block_ref")]` (`::option` for an
`Option`). It is saved under tag `BLOCK_REF`, and `Document::block_refs` finds every one, even
inside properties this build does not know. `folder.rs` and `canvas.rs` hold theirs this way.

References are recorded with every commit and drive the graph: backlinks, the `References` and
`Backrefs` queries editors watch, and the `KnowExists` access a member gets to a block something
they can see refers to. They are never for liveness.

Override `Root::references` only for references the content does not hold as ids, such as the
block URLs in a text body. Its result comes first, then the `BlockRef`s it did not name:

```rust
fn references(&self) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    block_url::parse_block_urls(&self.body)
        .into_iter()
        .map(|url| url.block)
        .filter(|block| seen.insert(*block))
        .collect()
}
```

Return each block once, in a deterministic order, and leave out ids that are not blocks (a
database's field ids). Override `Root::references_in` when a reference depends on the workspace,
as text does for block URLs (`guides/blocks/stack.md`, "Block URLs").

### Name

A block's name lives in its sealed metadata. Override `Root::name` when the content can say
something better than the type's display name:

```rust
fn name(&self) -> Option<String> {
    let title = self.title.trim().to_owned();
    (!title.is_empty()).then_some(title)
}
```

- Every save records it as the `INTRINSIC_NAME` metadata. A name set by hand (`NAME`) wins.
- Left at its default, `None`, the UI falls back to the type's display name.
- Anything else others want before the content loads goes in `Root::describe` (or
  `BlockContent::describe`), under a `MetaKey::derived` key. See `guides/blocks/in_the_app.md`,
  "Metadata".

### Children

If the type has a natural child that the Files sidebar can add, remove or swap by drag and drop,
override `Root::child_edit`. It turns a `ChildChange` into an ordinary edit and answers `None`
for a change the type does not support:

```rust
fn child_edit(&self, change: ChildChange) -> Option<Edit> {
    Some(match change {
        ChildChange::Add(block) => self.add_block(block),
        ChildChange::Delete(block) => self.remove_block(block),
        ChildChange::Replace { old, new } => self.repoint(old, new),
    })
}
```

- The app calls it through `be::change_child`, so the host makes the edit and the editor is not
  asked.
- Answer with an empty `Edit` for a child already in the requested state.
- Examples: `presentation.rs`, `folder.rs`, `hotbar.rs`.
- An editor whose references live in content it does not model this way (the text editor's block
  URLs) answers `Editor::on_replace_child` instead.
- The plugin's manifest says which of these edits the type accepts (`children`).

### Undo

A `Document` implements `Undo` already. It is enabled per type in `KINDS` with
`kind_with_history`. Undo belongs to the app's peer, so an editor gets it without writing
anything.

## 4. Export and register

Declare the module in `crates/be-block/src/lib.rs` and re-export the root and content type:

```rust
pub mod my_block;

pub use my_block::{MyBlock, MyBlockContent};
```

Add it to `KINDS` in `crates/block-app/src/be.rs`, the only list of content types the app opens,
copies, seeds and names:

```rust
kind_with_history::<be_block::MyBlockContent>(),
```

Use `kind::<C>()` for a type without undo. A block whose type is not in `KINDS` has no content in
the app: the host opens nothing for it and refuses content written to it.

Plugins reach the type through `block_editor_beui::be_block`, so an editor does not depend on
`be-block` itself.

## 5. Add model tests

Tests for a content type live in `crates/be-block/src/tests/`, one test per file named after its
function, declared in `crates/be-block/src/tests.rs` beside the shared imports and helpers such
as `edited`, which applies a list of edits to a starting content.

```text
crates/be-block/src/
  my_block.rs
  tests.rs
  tests/
    a_my_block_clears_only_the_done_items.rs
    two_my_blocks_merge_every_item_either_side_added.rs
```

```rust
use super::*;

#[test]
fn a_my_block_clears_only_the_done_items() {
    let (first, add_first) = MyBlock::add("one");
    let (_, add_second) = MyBlock::add("two");
    let content = edited(
        &MyBlockContent::default(),
        [add_first, add_second, MyBlock::set_done(first, true)],
    );

    let content = edited(&content, [content.root().clear_done()]);

    assert_eq!(content.root().items.len(), 1);
    assert_eq!(MyBlockContent::decode(&content.encode()), Ok(content));
}
```

- Test what the helpers mean: an edit made against content that has since changed, references,
  `child_edit`, a merge of two sides (`Merge::merge3`), and an undo (`Undo::step` then `revert`)
  when the type has one.
- Do not re-test lists or registers; the model is tested in `crates/be-model/src/tests/`. The
  round trip through a real server is tested in `crates/be-client/src/tests/`.
- Do not put tests in the production file and do not use `#[path]`.
- An editor's tests stand in for the host with `block_ui_test::BeuiTest`. It holds the content of
  the editor's block and of any block it watches through `WatchContent`, applies the `Operate`
  messages the editor sends and echoes them back as operations marked as its own, as the app's
  host does. `store()` hands a fixture the same `ContentStore` to read or write between runs.

## 6. Verify and add a UI

Run `./scripts/verify`. If the block needs a UI, continue with
`guides/plugins/adding_an_editor.md`.

## Modelling patterns

Almost every content type is a `Document`: the counter, checklist, calendar, browser tab, UI
settings, the three database types, presentation, hotbar, deterministic game, map, video, logic
game, logic grid, compiled logic, infinite canvas, pixel art and text. Patterns worth copying:

- **Point at an object, not an index.** The browser tab's current page is a register holding an
  `Option<ObjectId>` of an object in its history, so a push and a navigation made at once still
  agree on which page is current.
- **A tree as nested lists.** In the video, a clip attached to another is an object in that clip's
  list: reattaching is a move, a move that would make a cycle is refused by the model, and
  removing a clip takes what is attached to it.
- **Commands through `edit_for`.** An editor may speak in an operation enum as a command, and
  `edit_for` on the content turns it into an edit against the content as it is now. The video
  (`VideoOperation`, `VideoProject::edit_for`, reading a flattened `Video` for its timeline), the
  logic game, the logic grid, the canvas and pixel art work this way.
- **Diff the applied command.** The logic grid applies the command to the grid it reads back and
  writes the fields that differ, so components merge field by field. Its wires are stored as a set
  of unit-length segments and a set of wire ends, merged segment by segment, so a cut, an
  extension and a branch made at once all survive; they are joined back into wires, split at the
  stored ends, when read. Two components given the same id at once are both kept, the later read
  with a fresh id.
- **Order as list order.** The canvas keeps layering as the order of its entity list, so bringing
  an entity to the front is a move.
- **Pixels in a `Grid`.** Pixel art.
- **Ids as stable names.** The database schema's fields and enum options are objects, and their
  ids are what cells and enum values store, so renaming a field or option changes nothing that
  points at it.

## Files: Blob

A file does not fit a document: a small header and a payload that can be megabytes. `Blob<K>` is
that shape once, for any `BlobKind` that names a content type and a header. `ImageContent`,
`AudioContent`, `PdfContent`, `GameModuleContent` and `PaintSnapshotContent` are each a `Blob` of
their own kind.

- It encodes as a `Streamed` type, so a reader takes the header and then byte ranges.
- Its only live edit is `BlobOp::SetHeader` (the image editor records what it decoded that way);
  an offline merge takes whichever side changed it.
- The payload never travels as an operation, because a session relays operations in frames of at
  most 8 MB. It arrives as a commit, already chunked by be-store: a new block's first content
  through `SeedContent` or `CreateBlock`, a new file for an existing block through
  `ReplaceContent` (`guides/blocks/in_the_app.md`, "Creating, seeding and replacing").
- A blob kind bumps `BlobKind::SCHEMA` when its header changes so an old header would be read
  wrongly, which offers blocks saved before the change for reset (`guides/blocks/in_the_app.md`,
  "Blocks saved by another build"). A document's schema is derived from its structs.

Content that fits neither shape implements `BlockContent`, `LiveEdit` and `Merge` by hand in
`be-block`, and `Undo` if it wants one. Prefer a document whenever the content can be described
as one.

## The stored form

`Document::to_bytes` writes CBOR (`ciborium`) that names everything and holds nothing a live
session needs: no positions, tombstones or op history.

- The document is `{"format": N, "root": object}`.
- An object is a map of its properties by field name, plus `$kind` (the struct's kind) and, for an
  object in a `List`, `$id`: its `ObjectId` as a UUID (tag 37), so offline merges match objects
  across commits. A repeated id is given a fresh one on load.
- A register is its serde form in CBOR (structs as maps by field name, enum variants by name).
- A `Text` is bytes (any bytes, not only UTF-8).
- A `List` is an array of objects; a `Map` is an array of `[key, value]` pairs.
- The same CBOR is what registers hold in memory and what `set` sends, so a value read back is
  byte for byte what was saved.

## Changing the stored form

The format is meant to change without migrations. Until a type is frozen, an incompatible change
is also allowed: blocks saved before it are offered for reset (`guides/blocks/in_the_app.md`,
"Blocks saved by another build").

| Change | How |
|---|---|
| Add a field, a field inside a register's struct, an enum variant or a kind | Nothing: a missing property reads as its default. |
| Rename a field | `#[model(rename = "...")]` keeps the stored name when the Rust name changes. |
| Read an old name too | `#[model(alias = "old")]`. |
| Change a field's meaning | `#[model(from_old("old", convert))]` reads the old name through `fn convert(Old) -> New`. |
| Move data between objects | Bump the root's `#[model(format = N, migrate = path)]`; `migrate(old_format, &mut root)` rewrites the stored root before it is read. |
| Stop older builds editing an object they cannot fully read | `#[critical]` on the field. |

- Give every struct a register holds `#[serde(default)]`, and every field of an enum's struct
  variant `#[serde(default)]`, or a document saved before a field was added loses the whole
  register. `Kind::registers_accept_missing_fields` lists the registers that would.
- Old names are read, never written.
- Unknown data is kept: properties a build does not know stay on their object (`Object::kept`),
  are merged key by key and written back, so an older build never drops what a newer one wrote. A
  register whose bytes this build cannot decode reads as its default but keeps its bytes until it
  is set.
- A critical field set to something other than its default is saved inside tag `CRITICAL`. A
  build that does not know the field locks that object (`Document::is_locked`): its fields and
  lists refuse edits, though it can still be moved or removed whole. The rest of the document stays
  editable.
- A build refuses a format newer than its own.
- The content's references are the `BlockRef`s plus `Root::references`.

## Freezing a version

Freeze a type once its saved blocks must stay readable forever (`guides/blocks/frozen_formats.md`
lists what is frozen and what else freezing involves).

1. Set `Root::FROZEN` (or `BlobKind::FROZEN`). This turns off the stale-block handling for the
   type.
2. Call `be_model::schema::freeze(&document)`. It returns Rust source holding the type's registry
   (`Kind::describe`: every kind, property, shape, register default and old name) and the
   document's bytes.
3. Keep that source as a `frozen_*.rs` module of be-block's tests, tested with
   `schema::check_frozen::<Root>(REGISTRY, DOCUMENT)`. It fails when a frozen property is gone or
   retyped, a register default changed, the document no longer loads, or anything in it is no
   longer understood.
