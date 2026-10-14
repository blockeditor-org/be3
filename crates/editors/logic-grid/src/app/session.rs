use std::cell::RefCell;

use beui::reactive::{ReadSignal, Stored, WriteSignal, create_effect, create_signal, store};
use block_editor_beui::Editor;

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Session {
    editor: Editor,
    model: Stored<RefCell<LogicGridEditor>>,
    version: ReadSignal<u64>,
    set_version: WriteSignal<u64>,
    pub(super) pointer: ReadSignal<Option<[f32; 2]>>,
    pub(super) set_pointer: WriteSignal<Option<[f32; 2]>>,
    pub(super) debug_hover: ReadSignal<Option<DebugEntity>>,
    pub(super) set_debug_hover: WriteSignal<Option<DebugEntity>>,
    pub(super) graph_hover: ReadSignal<GraphHover>,
    pub(super) set_graph_hover: WriteSignal<GraphHover>,
}

impl Session {
    pub(super) fn new(editor: Editor) -> Self {
        let (version, set_version) = create_signal(0_u64);
        let (pointer, set_pointer) = create_signal(None);
        let (debug_hover, set_debug_hover) = create_signal(None);
        let (graph_hover, set_graph_hover) = create_signal(GraphHover::default());
        let session = Self {
            editor,
            model: store(RefCell::new(LogicGridEditor::live(editor))),
            version,
            set_version,
            pointer,
            set_pointer,
            debug_hover,
            set_debug_hover,
            graph_hover,
            set_graph_hover,
        };
        create_effect(move || {
            session.version.get();
            session.sync();
        });
        session
    }

    pub(super) fn editor(&self) -> Editor {
        self.editor
    }

    pub(super) fn editable(&self) -> bool {
        self.editor.editable().get_untracked()
    }

    pub(super) fn set_camera(&self, camera: Camera) {
        self.model.with(|model| model.borrow_mut().camera = camera);
    }

    pub(super) fn read<T>(&self, read: impl FnOnce(&LogicGridEditor) -> T) -> T {
        self.version.with(|_| ());
        self.model.with(|model| read(&model.borrow()))
    }

    pub(super) fn peek<T>(&self, read: impl FnOnce(&LogicGridEditor) -> T) -> T {
        self.model.with(|model| read(&model.borrow()))
    }

    pub(super) fn update<T>(&self, change: impl FnOnce(&mut LogicGridEditor) -> T) -> T {
        let result = self.model.with(|model| {
            let mut model = model.borrow_mut();
            let result = change(&mut model);
            model.settle();
            result
        });
        self.set_version.update(|version| *version += 1);
        result
    }

    pub(super) fn edit<T>(&self, change: impl FnOnce(&mut LogicGridEditor) -> T) -> Option<T> {
        self.editable().then(|| self.update(change))
    }

    fn sync(&self) {
        let client_id = self.editor.host().client_id();
        let changed = self.model.with(|model| {
            let mut model = model.borrow_mut();
            let changed = model.sync(true, client_id);
            let passed = model.take_challenge_passed();
            if passed && self.editable() {
                model.edit(LogicGridOperation::SetCompleted { completed: true });
            }
            if changed {
                model.settle();
            }
            changed || passed
        });
        if changed {
            self.set_version.update(|version| *version += 1);
        }
    }
}

impl LogicGridEditor {
    fn settle(&mut self) {
        if !self.loaded() {
            return;
        }
        self.update_simulation_preview();
        self.ensure_challenge_test();
    }
}
