use block_editor_beui::Editor;
use block_editor_beui::be_block::metadata::MAX_NAME_BYTES;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::{Key, KeyPhase};
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
        on_key(KeyAt::Window, KeyPhase::Capture, move |event| {
            let confirms = event
                .press()
                .is_some_and(|press| press.pressed && press.key == Key::Enter)
                && pending.with_untracked(Option::is_some);
            if confirms {
                pending.set(None);
            }
            confirms
        });
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
            self.editor
                .blocks()
                .set_name(*id, Some(clipped(name).to_owned()));
        }
        self.finish(key);
    }

    pub(crate) fn finish(&self, key: &RowKey) {
        if self.editing.get_untracked().as_ref() == Some(key) {
            self.editing.set(None);
        }
    }
}

fn clipped(name: &str) -> &str {
    let mut end = name.len().min(MAX_NAME_BYTES);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    &name[..end]
}
