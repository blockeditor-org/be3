# Adding a block

A block's type is its content type. The content lives in `crates/be-block`, as a `be-model` document that knows how to merge, undo and address its own parts; its UI belongs in a plugin under `crates/editors`. This guide covers the content. For how documents, edits, merging and undo work underneath, read `guides/the_new_block_stack.md` ("Adding a content type"); this guide does not repeat it.

## 1. Define the model

Create `crates/be-block/src/my_block.rs`. Describe the content as structs that derive `be_model::Model`, and do not write a merge, a rebase or an undo: the derive and `Document` provide them.

```rust
use be_model::{Anchor, Change, Document, Edit, List, Model, ObjectId};
use uuid::Uuid;

use crate::Root;

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

`CONTENT_TYPE` is the block type everywhere: it is what the server records for the block, what the app's registry is keyed by, and the `block_type` a plugin's manifest names. Once a block of it has been saved, changing it leaves that block with a type nothing opens.

A field is a register (any `Serialize + DeserializeOwned + Clone + PartialEq + Default` value, set as a whole), a `Count` whose concurrent changes add up, a `List<T>` of objects of another `Model` type, a `Map<K, V>` of per-key registers, or a `Grid<T>` of fixed-size cells. The stack guide says what each one does under a merge and an undo; choose by what two people changing it at once should end up with.

A block holds what it is, not what an editor makes of it. The image block keeps the file's bytes and a header saying what decoding them found - or why it failed - which the editor fills in the first time it draws the image. Nothing but the plugin then needs a decoder, and no other client has to decode the block to know its shape.

## 2. Write edits as helpers

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

`crates/be-block/src/counter.rs` and `checklist.rs` are the smallest complete examples; `calendar.rs` shows an update that only writes the fields that changed.

## 3. References

If the content holds the ids of other blocks, override `Root::references`. It is recorded with every commit and drives the graph: backlinks, the `References` and `Backrefs` queries editors watch, and the `KnowExists` access a member gets to a block something they can see refers to.

```rust
fn references(&self) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    self.slides
        .iter()
        .filter_map(|slide| slide.block)
        .filter(|block| seen.insert(*block))
        .collect()
}
```

A reference is a plain `Uuid` (or `Option<Uuid>`) field: `DatabaseView` holds `database: Option<Uuid>` and returns it, `Presentation` returns each slide's block. Return each block once, in a deterministic order, and leave out ids that are not blocks, such as a database's field ids. References are for navigation and access, never for liveness: a block stays alive because its parent chain reaches the root, not because something points at it.

## 4. Name

A block's name lives in its sealed metadata, which the server cannot read. Override `Root::name` when the content can say something better than the block type's display name:

```rust
fn name(&self) -> Option<String> {
    let title = self.title.trim().to_owned();
    (!title.is_empty()).then_some(title)
}
```

Whenever an editor that may edit the block sees a new revision, the app writes this as the block's automatic name (`be::name_implicitly`). It never overrides a name someone set by hand. Leave `name` at its default, `None`, and the UI falls back to the type's display name.

## 5. Children

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

The app calls it through `be::change_child`, so the host makes the edit itself and the editor is not asked. An empty `Edit` is the answer for a child already in the requested state. `presentation.rs`, `folder.rs` and `hotbar.rs` are the examples. An editor whose references live inside content it does not model this way - the text editor's block URLs - answers `Editor::on_replace_child` instead. The plugin's manifest says which of these edits the type accepts (`children`).

## 6. Undo

A `Document` implements `Undo` already: every edit records a step that is taken against the state before it and reverted against the state as it is now, so undo leaves alone what someone else changed since. It is enabled per type in the app's registry (below) with `kind_with_history`. Undo belongs to the app's peer, not to the editor, so an editor gets it without writing anything.

A content type that does not fit a document - text, or a file with a large payload (`Blob<K>`) - implements `BlockContent`, `LiveEdit` and `Merge` by hand in `be-block`, and `Undo` too if it wants one. Prefer a document whenever the content can be described as one.

## 7. Export and register

Declare the module in `crates/be-block/src/lib.rs` and re-export the root and its content type:

```rust
pub mod my_block;

pub use my_block::{MyBlock, MyBlockContent};
```

Then add it to `KINDS` in `crates/block-app/src/be.rs`, which is the only list of the content types the app opens, copies, seeds and names:

```rust
kind_with_history::<be_block::MyBlockContent>(),
```

Use `kind::<C>()` for a type without undo. A block whose content type is not in `KINDS` has no content in the app: the host opens nothing for it and refuses content written to it.

Plugins reach the type through `block_editor_beui::be_block`, so an editor does not depend on `be-block` itself.

## 8. Add model tests

Tests for a content type live in `crates/be-block/src/tests/`, one test per file named after its function, declared in `crates/be-block/src/tests.rs` beside the shared imports and helpers such as `edited`, which applies a list of edits to a starting content:

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

Test what the helpers mean: an edit made against content that has since changed, references, `child_edit`, a merge of two sides (`Merge::merge3`), and an undo (`Undo::step` then `revert`) when the type has one. The model itself is tested in `crates/be-model`; do not re-test lists or registers here. Do not put tests in the production file and do not use `#[path]`.

## 9. Verify

`./scripts/buck run //:verify` runs the full check from the workspace root: it applies the project's autofixes and runs every lint and test. CI runs the same on a pull request and pushes whatever it changes to the branch.

```text
./scripts/buck run //:verify
```

If the block needs a UI, continue with the [plugin editor guide](adding_a_plugin_editor.md).
