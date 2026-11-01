use std::cell::RefCell;

use beui::reactive::*;
use block_editor_beui::Editor;

use super::*;

#[derive(Clone, Copy)]
pub(super) struct Session {
    editor: Editor,
    model: Stored<RefCell<LogicGridEditor>>,
    version: Signal<u64>,
    pub(super) pointer: Signal<Option<[f32; 2]>>,
    pub(super) debug_hover: Signal<Option<DebugEntity>>,
    pub(super) graph_hover: Signal<GraphHover>,
    pub(super) configured_at: NodeRef,
}

impl Session {
    pub(super) fn new(editor: Editor) -> Self {
        let version = create_signal(0_u64);
        let pointer = create_signal(None);
        let debug_hover = create_signal(None);
        let graph_hover = create_signal(GraphHover::default());
        let session = Self {
            editor,
            model: store(RefCell::new(LogicGridEditor::live(editor))),
            version,
            pointer,
            debug_hover,
            graph_hover,
            configured_at: NodeRef::new(),
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

    pub(super) fn set_camera(&self, camera: CanvasView) {
        self.model.with(|model| model.borrow_mut().camera = camera);
    }

    pub(super) fn read<T>(&self, read: impl FnOnce(&LogicGridEditor) -> T) -> T {
        self.version.with(|_| ());
        self.model.with(|model| read(&model.borrow()))
    }

    pub(super) fn read_untracked<T>(&self, read: impl FnOnce(&LogicGridEditor) -> T) -> T {
        self.model.with(|model| read(&model.borrow()))
    }

    pub(super) fn update<T>(&self, change: impl FnOnce(&mut LogicGridEditor) -> T) -> T {
        let result = self.model.with(|model| {
            let mut model = model.borrow_mut();
            let result = change(&mut model);
            model.settle();
            result
        });
        self.version.update(|version| *version += 1);
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
            self.version.update(|version| *version += 1);
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
