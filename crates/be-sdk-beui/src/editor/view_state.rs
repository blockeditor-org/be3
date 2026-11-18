//! What an editor keeps in its view block (guides/blocks/workspaces.md, View state).

use std::marker::PhantomData;

use be_block_content::{EditorViewContent, ViewStateKind};
use std::cell::Cell;

use beui::reactive::*;

use super::Editor;

/// The `K` kept in the editor's view block (`Editor::view_state`).
pub struct ViewStateHandle<K> {
    editor: Editor,
    kind: PhantomData<fn() -> K>,
}

impl<K> Clone for ViewStateHandle<K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K> Copy for ViewStateHandle<K> {}

impl<K: ViewStateKind> ViewStateHandle<K> {
    pub(super) fn new(editor: Editor) -> Self {
        Self {
            editor,
            kind: PhantomData,
        }
    }

    /// The value kept, tracked: `Loading` until the view block is read, `Ok(None)` when nothing
    /// is kept or the editor has no view block, and `Failed` when what is kept cannot be read
    /// (or the view block cannot). Most callers start afresh on `Failed` and write over it, as
    /// `keep` does; one whose state is worth recovering by a later build (the shell's layout)
    /// keeps it until the person resets it.
    pub fn get(&self) -> Ready<Option<K>> {
        let Some(block) = self.editor.view_block() else {
            return Ok(None);
        };
        let kept = self
            .editor
            .projection_of::<EditorViewContent>(Some(block))
            .read(|view| view.root().view_state::<K>())
            .map_err(|pending| match pending {
                Pending::Failed(error) => {
                    Pending::Failed(format!("the view block cannot be read: {error}"))
                }
                loading => loading,
            })?;
        kept.transpose().map_err(Pending::Failed)
    }

    pub fn get_untracked(&self) -> Ready<Option<K>> {
        untrack(|| self.get())
    }

    /// Keeps `value` (none removes it) at once, unless it is what is kept already. Write on
    /// every change: a burst of writes is folded into one where it is stored, so nothing here
    /// is ever debounced.
    pub fn set(&self, value: Option<K>) {
        self.editor.state().core.set_view_state(value.as_ref());
    }

    /// Puts back what is kept and then keeps what is shown: calls `restore` once, with what is
    /// kept the first time it is read (nothing when it cannot be read), and from then on writes
    /// `current()` whenever what it reads changes.
    pub fn keep(
        &self,
        restore: impl FnOnce(Option<K>) + 'static,
        current: impl Fn() -> K + 'static,
    ) {
        let handle = *self;
        let restored = create_signal(false);
        let restore = Cell::new(Some(restore));
        create_binding(move || {
            if restored.get_untracked() {
                return;
            }
            let kept = match handle.get() {
                Ok(kept) => kept,
                Err(Pending::Loading) => return,
                Err(Pending::Failed(_)) => None,
            };
            if let Some(restore) = restore.take() {
                untrack(|| restore(kept));
            }
            restored.set(true);
        });
        create_effect(move || {
            if !restored.get() {
                return;
            }
            let value = current();
            untrack(|| handle.set(Some(value)));
        });
    }
}
