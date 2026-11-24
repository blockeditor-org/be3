use be_sdk_beui::Editor;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::block_menu::rename_block;
use be_sdk_beui::beui::{Key, KeyPhase};
use uuid::Uuid;

use super::rows::{RowKey, Tree as FileTree};

#[derive(Clone, Copy)]
pub(crate) struct Renaming {
    editor: Editor,
    tree: FileTree,
    editing: Signal<Option<RowKey>>,
    pending: Signal<Option<Uuid>>,
}

impl Renaming {
    pub(crate) fn new(editor: Editor, tree: FileTree, keys: Memo<Vec<RowKey>>) -> Self {
        let renaming = Self {
            editor,
            tree,
            editing: create_signal(None),
            pending: create_signal(None),
        };
        let (pending, rows) = (renaming.pending, tree.rows());
        create_effect(move || {
            let Some(id) = pending.get() else {
                return;
            };
            let editable = rows.with(|rows| {
                rows.iter()
                    .filter(|row| row.id == Some(id) && row.can_edit)
                    .map(|row| row.key.clone())
                    .collect::<Vec<_>>()
            });
            let shown = keys.with(|keys| keys.iter().find(|key| editable.contains(key)).cloned());
            if let Some(key) = shown {
                untrack(|| renaming.start(key));
            }
        });
        // Another block taking the keyboard is the person moving on: a new block's name is
        // no longer asked for once its row shows. A started rename's field ends itself, as
        // left, when its window loses the keyboard.
        let keyboard = editor.has_keyboard();
        create_effect(move || {
            if !keyboard.get() && pending.with_untracked(Option::is_some) {
                pending.set(None);
            }
        });
        on_key(
            KeyAt::Window,
            KeyPhase::Capture,
            KeyReveal::Focus,
            move |event| {
                let confirms = event
                    .press()
                    .is_some_and(|press| press.pressed && press.key == Key::Enter)
                    && pending.with_untracked(Option::is_some);
                if confirms {
                    pending.set(None);
                }
                confirms
            },
        );
        renaming
    }

    pub(crate) fn editing(&self) -> ReadSignal<Option<RowKey>> {
        self.editing.read_only()
    }

    pub(crate) fn start(&self, key: RowKey) {
        let editable = self
            .tree
            .rows()
            .with_untracked(|rows| rows.iter().any(|row| row.key == key && row.can_edit));
        if editable {
            self.pending.set(None);
            self.editing.set(Some(key));
        }
    }

    pub(crate) fn rename_when_shown(&self, id: Uuid, parent: Option<Uuid>) {
        if let Some(parent) = parent {
            self.tree
                .toggle(&RowKey::Block(vec![parent]), Some(parent), false);
        }
        self.editing.set(None);
        self.pending.set(Some(id));
    }

    pub(crate) fn rename(&self, key: &RowKey, name: &str) {
        if let RowKey::Block(path) = key
            && let Some(id) = path.last()
        {
            rename_block(self.editor, *id, name);
        }
        self.finish(key);
    }

    pub(crate) fn finish(&self, key: &RowKey) {
        if self.editing.get_untracked().as_ref() == Some(key) {
            self.editing.set(None);
        }
    }
}
