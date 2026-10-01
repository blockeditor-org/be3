use std::rc::Rc;

pub use beui_core::file_picker::{FileFilter, FilePick, PickedFile};

use crate::reactive::{
    ReadSignal, ScopeContext, WriteSignal, create_signal, owner_scope, with_document,
};

pub fn pick_file(filter: FileFilter, picked: impl FnOnce(FilePick) + 'static) {
    with_document(|document| document.pick_file(filter, picked));
}

#[derive(Clone)]
pub struct FilePicker {
    picking: ReadSignal<bool>,
    set_picking: WriteSignal<bool>,
    picked: Rc<dyn Fn(Result<PickedFile, String>)>,
    owner: Option<ScopeContext>,
}

pub fn create_file_picker(picked: impl Fn(Result<PickedFile, String>) + 'static) -> FilePicker {
    let (picking, set_picking) = create_signal(false);
    FilePicker {
        picking,
        set_picking,
        picked: Rc::new(picked),
        owner: owner_scope(),
    }
}

impl FilePicker {
    pub fn picking(&self) -> ReadSignal<bool> {
        self.picking.clone()
    }

    pub fn open(&self, filter: FileFilter) {
        if self.picking.get_untracked() {
            return;
        }
        self.set_picking.set(true);
        let owner = self.owner.clone();
        let set_picking = self.set_picking.clone();
        let picked = Rc::clone(&self.picked);
        pick_file(filter, move |pick| {
            if owner.as_ref().is_some_and(|owner| !owner.is_alive()) {
                return;
            }
            set_picking.set(false);
            match pick {
                Ok(Some(file)) => picked(Ok(file)),
                Ok(None) => {}
                Err(error) => picked(Err(error)),
            }
        });
    }
}
