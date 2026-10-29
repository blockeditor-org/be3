pub use ui_types::files::{FileSave, SavedFile};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileSaveId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSaveRequest {
    pub id: FileSaveId,
    pub file: SavedFile,
}

#[derive(Clone)]
pub struct FileSaveAnswer {
    id: FileSaveId,
    sender: std::sync::mpsc::Sender<(FileSaveId, FileSave)>,
    waker: Option<crate::app::Waker>,
}

impl FileSaveAnswer {
    pub fn new(
        id: FileSaveId,
        sender: std::sync::mpsc::Sender<(FileSaveId, FileSave)>,
        waker: Option<crate::app::Waker>,
    ) -> Self {
        Self { id, sender, waker }
    }

    pub fn id(&self) -> FileSaveId {
        self.id
    }

    pub fn send(self, save: FileSave) {
        let _ = self.sender.send((self.id, save));
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }
}
