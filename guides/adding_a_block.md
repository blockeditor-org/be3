# Adding a block

A block's type is its content type, and an app brings it: the content is a
`be-block-model` document in the app's *types crate*, which knows how to merge,
undo and address its own parts and is built into the app's type module
(guides/type_modules.md); the app's manifest declares it. Its UI is the app's
editor (guides/adding_an_app.md). How documents, edits, merging and undo work
underneath is in guides/blocks/content_types.md.
`crates/be-type-checklist` is the example to copy.

## 1. Make the types crate

The types of `crates/be-app-my-block` live in `crates/be-type-my-block` (add it
to the root `Cargo.toml`'s members and run `./scripts/buck run //:buckify`). It
is a plain library the editor, its tests and other crates depend on, and its
`:module` build is the type module the host runs. Copy checklist's
`Cargo.toml` (`crate-type = ["cdylib", "rlib"]`, a `module` feature) and
`BUCK` (a `cargo_library`, a `wasm` library with the `module` feature, and a
`wasm32_module` named `module`).

`src/lib.rs` lists the types and exports the guest only under `module`, so a
crate depending on the library (an editor compiled to wasm) does not get a
second set of guest exports, which would clash:

```rust
mod my_block;

pub use my_block::{MyBlock, MyBlockContent, MyItem};

be_sdk_type_guest::be_block_types![MyBlockContent { undo }];

#[cfg(feature = "module")]
be_sdk_type_guest::type_module!(BlockTypes);
```

Leave out `undo` for a type without undo; add `file` for a type with a
plain-file form, given by implementing `DocumentFile` on its root
(`crates/be-type-text/src/text.rs`) or `BlobFile` on its blob kind
(`crates/be-type-image/src/image.rs`).

## 2. Define the model

Describe the content as structs deriving `be_block_model::Model`; the derive
and `Document` provide merge, rebase and undo.

```rust
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
    // A new, permanent UUID. Never reuse another content type's.
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);
}

pub type MyBlockContent = Document<MyBlock>;
```

`CONTENT_TYPE` is the block type everywhere: the server records it, the host's
registry is keyed by it, the manifest declares the type under it and editors
`edits` it. Changing it after a block is saved leaves that block with a type
nothing opens.

A `kind` is saved with every object; the stack guide's "The stored form" says
how fields are saved and how to add, rename and retire them without a
migration. Choose each field's kind by what two people changing it at once
should get: a register (any serde value, set whole; give its structs
`#[serde(default)]`), a `Count` (concurrent changes add up), a `List<T>` of
objects, a `Map<K, V>` of per-key registers, or a `Grid<T>` of cells.

The content is what the block is, not what an editor makes of it
(decisions/block_content_is_what_the_block_is.md): the image block keeps the
file's bytes and what decoding them found, which the editor fills in the
first time it draws the image.

## 3. Write edits as helpers

The derive gives every field a typed constant (`MyItem::TEXT`,
`MyBlock::ITEMS`) that builds changes. Wrap them in helpers returning
`be_block_model::Edit`, so an editor calls
`content.operate(MyBlock::set_done(id, true))`:

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

    pub fn clear_done(&self) -> Edit {
        self.items
            .iter()
            .filter(|item| item.done)
            .map(|item| Change::remove(item.id))
            .collect()
    }
}
```

An edit names an object by its `ObjectId`, never a position, so two people
removing "the second item" remove the same one. A helper that creates an
object returns its id. Editors key their rows on those ids, which keeps a row
alive across an edit to its neighbour (guides/reactive.md). A helper that
reads the current content takes `&self`. `BrowserTab::replace`
(`crates/be-type-browser-tab`) shows an update writing only changed fields.

## 4. References

Store each id of another block as a `be_block_model::BlockRef` (or mark a
`Uuid` in a register with
`#[serde(with = "be_block_model::references::block_ref")]`). The content's
references are then found wherever they are stored and recorded with every
commit; they drive backlinks, the `References` and `Backrefs` queries, and
the `KnowExists` access a member gets to a block something they can see
refers to.

Override `Root::references` only for references not held as ids, such as the
block URLs in a text body; return each block once, in a deterministic order.
References serve navigation and access, not liveness: a block stays alive
because its parent chain reaches the root.

## 5. Name

Override `Root::name` when the content can name the block better than the
type's display name (a non-empty title). Every save records it as the
`INTRINSIC_NAME` metadata; a name set by hand (`NAME`) wins. Anything else
others want before the content loads goes in `BlockContent::describe`, under
a `MetaKey::derived` key.

## 6. Children

If the type has children the Files sidebar can add, remove, swap or reorder,
override `Root::child_edit`, which turns a `ChildChange` into an ordinary edit
(`None` for a change the type does not take; an empty `Edit` for one already
done). The host calls it through the block's replica, without asking the
editor. `crates/be-type-presentation/src/presentation.rs` and
`crates/be-type-workspace/src/folder.rs` (which reorders) are examples. A type
that takes `Reorder` keeps its children in order, so it overrides
`Root::references` to list them in that order too: the file tree lists a
block's children in the order of its references, and be-block-model's own
reference index keeps the order blocks first appeared in. The manifest's
`children` says which of these edits the type accepts.

## 7. Undo

A `Document` implements `Undo`: each step is reverted against the state as it
is now, so undo leaves alone what others changed since. It is enabled per type
with `undo` (section 1), and belongs to the block's replica, so an editor gets
it without writing anything.

Content that does not fit a document (a large file, `Blob<K>`) implements
`BlockContent`, `LiveEdit`, `Merge` and optionally `Undo` by hand. Its
constructors and constants go on the kind (`PdfFile::from_file`), since a
types crate cannot add methods to be-block's `Blob<K>`. Bump
`BlobKind::SCHEMA` when the header changes so an old one would be misread;
that offers blocks saved before the change for reset
(guides/blocks/host.md, Blocks saved by another build).

## 8. Declare it in the manifest

```json
"block_types": {
  "my_block": {
    "id": "00000000-0000-0000-0000-000000000001",
    "display_name": "My block",
    "icon": "",
    "color": [203, 91, 125],
    "children": {"add": true, "delete": true, "replace": true},
    "press": "Open",
    "module": "my_block_types.wasm"
  }
}
```

`id` is the `CONTENT_TYPE`; `display_name`, `icon` (a Lucide codepoint) and
`color` are what every label, the add menu and the file tree show, whichever
editor opens the block. The `be_sdk_manifest` types document the rest
(`children`, `press`, `exported_whole`, `plain_file`). Every type names its
module, and all of an app's types name the same one: the host runs one
instance per app. Then give the editor's `app()` in its BUCK file
`types = "//crates/be-type-my-block:module"`, which stages the module beside
the app.

The host refuses an app declaring a type another app or the host already has.
The type is known from its manifest before its module starts. The editor
depends on the types crate for its typed content; `be_sdk_beui::be_block`
gives it the traits.

## 9. Add model tests

Tests live in the types crate's `src/tests/`, one per file (verify splits
them), with shared helpers such as `edited` in `src/tests.rs`:

```rust
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

Test what the helpers mean: an edit against content that has since changed,
references, `child_edit`, a three-way merge (`Merge::merge3`) and undo. The
model itself is tested in `crates/be-block-model`.

Two registrations trip agents up:

- An editor test registers a type without undo the first time it names it by
  Rust type. A test that undoes, or holds blocks of a type it never names (an
  embedded child), registers them first with
  `test.store().register(my_block_types::registrations())`.
- The unit tests of be-host, be-tool-cli, be-block-sync and be-block-frozen
  run every app's module in process from `be_type_bundle::sources()`: add the
  new crate to `sources()` and `knows()` in `crates/be-type-bundle`, named by
  the app's manifest id. A module's session tag is a hash of that name, so
  another peer in the same test joins be-host's session only with the same
  sources in the same order.

## Types in be-block

`crates/be-block` holds only the types the host applies itself and the
libraries every type builds on. A frozen type would keep its `frozen_*.rs`
tests in its types crate (guides/frozen_formats.md); none is frozen yet.

If the block needs a UI, continue with [adding_an_app.md](adding_an_app.md).
