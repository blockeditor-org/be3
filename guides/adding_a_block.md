# Adding a block

A block's type is its content type, and a plugin brings it: the content is a `be-model` document in the plugin's *types crate*, which knows how to merge, undo and address its own parts and is built into the plugin's type module (guides/type_modules.md), and the plugin's manifest declares it. Its UI is the plugin's editor under `crates/editors`. This guide covers the type. For how documents, edits, merging and undo work underneath, read `guides/the_new_block_stack.md` ("Adding a content type"); this guide does not repeat it. `crates/editors/checklist/types` is the example to copy.

## 1. Make the types crate

A plugin `crates/editors/my-block` keeps its types in `crates/editors/my-block/types`, a crate of its own (`my-block-types`, added to the root `Cargo.toml`'s members; run `./scripts/buck run //:buckify` after). It is a plain library that the editor, its tests and any other crate depend on for the typed content, and its `:module` build is the type module the app runs:

```toml
[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
be-block = { path = "../../../be-block" }
be-model = { path = "../../../be-model" }
block-type-guest = { path = "../../../block-type-guest" }
uuid = "1"

[features]
module = []
```

```python
cargo_library()

cargo_test()

cargo_library(
    name = "wasm",
    extra_features = ["module"],
    preferred_linkage = "shared",
    target_compatible_with = ["prelude//cpu/constraints:cpu[wasm32]"],
)

wasm32_module(
    name = "module",
    library = ":wasm",
    module = "my_block_types",
    visibility = ["PUBLIC"],
)
```

`src/lib.rs` lists the crate's types in `be_block_types!` and exports the guest only under the `module` feature, so a crate that depends on the library, an editor compiled to wasm among them, does not get a second set of guest exports:

```rust
mod my_block;

pub use my_block::{MyBlock, MyBlockContent, MyItem};

block_type_guest::be_block_types![MyBlockContent { undo }];

#[cfg(feature = "module")]
block_type_guest::abi::guest!(BlockTypes);
```

Leave out `{ undo }` for a type without undo, and add `file` for one with a plain-file form, which the crate gives by implementing `block_type_guest::DocumentFile` on its root (`crates/editors/text-block/types/src/text.rs`) or `BlobFile` on its blob kind (`crates/editors/image-block/types/src/image.rs`); guides/type_modules.md says why not `FileForm` itself. The macro defines `registrations()`, `block_types()` (the `Module`) and `BlockTypes`, the guest.

## 2. Define the model

Create `src/my_block.rs` in the types crate. Describe the content as structs that derive `be_model::Model`, and do not write a merge, a rebase or an undo: the derive and `Document` provide them.

```rust
use be_block::Root;
use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct MyBlock {
    pub title: String,
    pub items: List<MyItem>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
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

`CONTENT_TYPE` is the block type everywhere: it is what the server records for the block, what the app's registry is keyed by, the `id` the manifest declares the type under, and what an editor's manifest says it `edits`. Once a block of it has been saved, changing it leaves that block with a type nothing opens.

Give each `Model` struct a kind, `#[model(kind = "my_block.item")]`: it is saved with every object, and the stack guide's "The stored form" says how fields are saved and how to add, rename and retire them without a migration. A field is a register (any `Serialize + DeserializeOwned + Clone + PartialEq + Default` value, set as a whole; give its structs `#[serde(default)]`), a `Count` whose concurrent changes add up, a `List<T>` of objects of another `Model` type, a `Map<K, V>` of per-key registers, or a `Grid<T>` of fixed-size cells. The stack guide says what each one does under a merge and an undo; choose by what two people changing it at once should end up with.

A block holds what it is, not what an editor makes of it. The image block keeps the file's bytes and a header saying what decoding them found - or why it failed - which the editor fills in the first time it draws the image. Nothing but the plugin then needs a decoder, and no other client has to decode the block to know its shape.

## 3. Write edits as helpers

The derive gives every field a typed constant (`MyItem::TEXT`, `MyBlock::ITEMS`) that builds the changes an `Edit` is made of: `set` for a register, `add` for a `Count`, `put` for a key of a `Map`, `insert` and `move_into` for a `List`, and `Change::remove` for any object. Wrap them in helpers that return `be_model::Edit`, so an editor calls `content.operate(MyBlock::set_done(id, true))` and never assembles changes itself:

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

Every object in a `List` has an `ObjectId`, and an edit names the object, never a position: two people removing what they each saw as the second item remove the same one, and an insert at the head renumbers nothing. A helper that creates an object returns its id, so the caller can address it straight away. Editors key their rows on those ids, which is what lets a row survive an edit to the item next to it - see `guides/reactive.md`. A helper that needs the current content to decide what to write (`clear_done`, `Counter::reset`) takes `&self`; one that does not is an associated function.

`crates/editors/checklist/types/src/checklist.rs` is the smallest complete example; `crates/editors/calendar/types/src/calendar.rs` shows an update that only writes the fields that changed.

## 4. References

If the content holds the ids of other blocks, store each as a `be_model::BlockRef` (or mark a `Uuid` inside a register with `#[serde(with = "be_model::references::block_ref")]`): the content's references are then every one of them, found wherever it is stored. References are recorded with every commit and drive the graph: backlinks, the `References` and `Backrefs` queries editors watch, and the `KnowExists` access a member gets to a block something they can see refers to. be-block's `folder.rs` and `crates/editors/infinite-canvas/types/src/canvas.rs` hold theirs this way.

Override `Root::references` only for references the content does not hold as ids, such as the block URLs in a text block's body; its result comes first, followed by the `BlockRef`s it did not name.

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

Return each block once, in a deterministic order, and leave out ids that are not blocks, such as a database's field ids. References are for navigation and access, never for liveness: a block stays alive because its parent chain reaches the root, not because something points at it.

## 5. Name

A block's name lives in its sealed metadata, which the server cannot read. Override `Root::name` when the content can say something better than the block type's display name:

```rust
fn name(&self) -> Option<String> {
    let title = self.title.trim().to_owned();
    (!title.is_empty()).then_some(title)
}
```

Every save records this as the block's `INTRINSIC_NAME` metadata, and the app shows it while an editor has the block open (`block_app_stack::describe_implicitly`). A name someone set by hand (`NAME`) takes precedence. Anything else the content can say about itself that others want before it loads goes in `BlockContent::describe`, under a `MetaKey::derived` key. Leave `name` at its default, `None`, and the UI falls back to the type's display name.

## 6. Children

If the type has a natural notion of a child that the Files sidebar can add, remove or swap by drag and drop, override `Root::child_edit`. It turns a `ChildChange` into an ordinary edit, and answers `None` for a change the type does not support:

```rust
fn child_edit(&self, change: ChildChange) -> Option<Edit> {
    Some(match change {
        ChildChange::Add(block) => self.add_block(block),
        ChildChange::Delete(block) => self.remove_block(block),
        ChildChange::Replace { old, new } => self.repoint(old, new),
    })
}
```

The app calls it through `block_app_stack::change_child`, which the block's replica turns into an edit (`Change::Child`), so the editor is not asked. An empty `Edit` is the answer for a child already in the requested state. `crates/editors/presentation/types/src/presentation.rs` and be-block's `folder.rs` and `hotbar.rs` are the examples. An editor whose references live inside content it does not model this way - the text editor's block URLs - answers `Editor::on_replace_child` instead. The type's manifest entry says which of these edits it accepts (`children`, below).

## 7. Undo

A `Document` implements `Undo` already: every edit records a step that is taken against the state before it and reverted against the state as it is now, so undo leaves alone what someone else changed since. It is enabled per type in `be_block_types!` (section 1) with `undo`. Undo belongs to the block's replica, not to the editor, so an editor gets it without writing anything.

A content type that does not fit a document - a file with a large payload (`Blob<K>`) - implements `BlockContent`, `LiveEdit` and `Merge` by hand, and `Undo` too if it wants one. Prefer a document whenever the content can be described as one. A blob type's constructor and constants go on its kind (`PdfFile::from_file`, `GameModuleFile::FILE_EXTENSIONS`): `Blob<K>` is be-block's, so a types crate cannot add methods to it. A blob kind bumps `BlobKind::SCHEMA` when its header changes so that an old header would be read wrongly, which offers its blocks saved before the change for reset (guides/the_new_block_stack.md, Blocks saved by another build); a document's schema is derived from its structs.

## 8. Declare it in the manifest

The plugin's `manifest.json` declares each type it brings under `block_types`, next to `editors`, keyed by a name unique within the plugin:

```json
"block_types": {
  "my_block": {
    "id": "00000000-0000-0000-0000-000000000001",
    "display_name": "My block",
    "icon": "\ue1d0",
    "color": [203, 91, 125],
    "children": {"add": true, "delete": true, "replace": true},
    "press": "Open",
    "module": "my_block_types.wasm"
  }
}
```

`id` is the `CONTENT_TYPE`. `display_name`, `icon` (a Lucide codepoint) and `color` are what the add menu, the file tree and every label show for a block of the type, whichever editor opens it. `children` (each `false` by default) is which structural edits it takes, `press` is `Open` (the default) or `Zoom`, which the file tree does to a block of it instead of opening it (a folder), `exported_whole` (`false` by default) makes exporting any child of a block of the type as a block file export the block instead (a database view, so its database never leaves without it), and `module` is the plugin's type module, the file `:module` builds. All of a plugin's types name the same module: the host runs one instance of it per plugin. Every type names it: the manifest is refused without one. Then give the editor's `editor()` call in its BUCK file `types = "//crates/editors/my-block/types:module"`, which stages the module beside the plugin's own, with its `.cwasm` and its SHA-256, the tag of its sessions.

The host builds its catalog of block types from every installed manifest and the types it applies itself, and refuses a plugin that declares a type another plugin or the host already has, or whose editor `edits` (or whose template makes) a type nobody declares. The type is known from its manifest before its module has started.

The editor depends on the types crate (`my-block-types = { path = "types" }`) for its typed content, as `crates/editors/checklist` does; `block_editor_beui::be_block` still gives it the traits and `ObjectId`.

## 9. Add model tests

Tests for a content type live in the types crate's `src/tests/`, one test per file named after its function, declared in `src/tests.rs` beside the shared imports and helpers such as `edited`, which applies a list of edits to a starting content:

```text
crates/editors/my-block/types/src/
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

Test what the helpers mean: an edit made against content that has since changed, references, `child_edit`, a merge of two sides (`Merge::merge3`), and an undo (`Undo::step` then `revert`) when the type has one. The model itself is tested in `crates/be-model`; do not re-test lists or registers here. An editor test (block_ui_test) hosts blocks in an in-process type module that already has the host's types (`crates/host-types`), and registers a type, without undo, the first time the test holds or reads one by its Rust type; a test that undoes, or that has blocks of a plugin's type it never names (an embedded child), registers them first with `test.store().register(my_block_types::registrations())`.

The unit tests of block-app, be-cli, be-sync and be-frozen run every plugin's module in process from `first_party_types::sources()` (`crates/first-party-types`): add the new types crate to its `sources()` and `knows()`, named by the plugin's manifest id. An in-process module's session tag is a hash of its name, so another peer in the same test (a terminal host) joins the app's session only under the same name; give such a peer the same sources, in the same order. Do not put tests in the production file and do not use `#[path]`.

## Moving a type out of be-block

`crates/be-block` holds only the types the host applies itself and the libraries every type builds on (`Root`, `Blob`, metadata, settling, history); a block type lives in its plugin's types crate, and a type that a frozen format covers keeps its `frozen_*.rs` tests there (guides/frozen_formats.md). be's `type_name` falls back to the manifest's display name, so a new type needs no entry there.

## 10. Verify

`./scripts/verify` runs the full check from the workspace root: it applies the project's autofixes and runs every lint and test. CI runs the same on a pull request and pushes whatever it changes to the branch.

```text
./scripts/verify
```

If the block needs a UI, continue with the [plugin editor guide](adding_a_plugin_editor.md).
