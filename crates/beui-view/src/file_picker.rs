use std::rc::Rc;

pub use crate::types::files::{FileFilter, FilePick, PickedFile};

use crate::reactive::{Get, ReadSignal, ScopeContext, Set, Signal, create_signal, owner_scope};

pub fn pick_file(filter: FileFilter, picked: impl FnOnce(FilePick) + 'static) {
    crate::core::files::pick_file(filter, picked);
}

#[derive(Clone)]
pub struct FilePicker {
    picking: Signal<bool>,
    picked: Rc<dyn Fn(Result<PickedFile, String>)>,
    owner: Option<ScopeContext>,
}

pub fn create_file_picker(picked: impl Fn(Result<PickedFile, String>) + 'static) -> FilePicker {
    let picking = create_signal(false);
    FilePicker {
        picking,
        picked: Rc::new(picked),
        owner: owner_scope(),
    }
}

impl FilePicker {
    pub fn picking(&self) -> ReadSignal<bool> {
        self.picking.read_only()
    }

    pub fn open(&self, filter: FileFilter) {
        if self.picking.get_untracked() {
            return;
        }
        self.picking.set(true);
        let owner = self.owner.clone();
        let picking = self.picking;
        let picked = Rc::clone(&self.picked);
        pick_file(filter, move |pick| {
            if owner.as_ref().is_some_and(|owner| !owner.is_alive()) {
                return;
            }
            picking.set(false);
            match pick {
                Ok(Some(file)) => picked(Ok(file)),
                Ok(None) => {}
                Err(error) => picked(Err(error)),
            }
        });
    }
}
