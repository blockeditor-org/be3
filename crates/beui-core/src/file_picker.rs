pub use ui_types::files::{FileFilter, FilePick, PickedFile};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FilePickId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilePickRequest {
    pub id: FilePickId,
    pub filter: FileFilter,
}

#[derive(Clone)]
pub struct FilePickAnswer {
    id: FilePickId,
    sender: std::sync::mpsc::Sender<(FilePickId, FilePick)>,
    waker: Option<crate::app::Waker>,
}

impl FilePickAnswer {
    pub fn new(
        id: FilePickId,
        sender: std::sync::mpsc::Sender<(FilePickId, FilePick)>,
        waker: Option<crate::app::Waker>,
    ) -> Self {
        Self { id, sender, waker }
    }

    pub fn id(&self) -> FilePickId {
        self.id
    }

    pub fn send(self, pick: FilePick) {
        let _ = self.sender.send((self.id, pick));
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }
}
