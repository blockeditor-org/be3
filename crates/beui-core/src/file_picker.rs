#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
    pub mime_types: Vec<String>,
}

impl FileFilter {
    pub fn new(name: &str, extensions: &[&str], mime_types: &[&str]) -> Self {
        let owned = |values: &[&str]| values.iter().map(|value| (*value).to_owned()).collect();
        Self {
            name: name.to_owned(),
            extensions: owned(extensions),
            mime_types: owned(mime_types),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickedFile {
    pub name: String,
    pub data: Vec<u8>,
}

pub type FilePick = Result<Option<PickedFile>, String>;

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
    pub(crate) fn new(
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
