#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClipboardReadId(pub u64);

pub type ImagePaste = crate::file_picker::FilePick;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImagePasteId(pub u64);

#[derive(Clone)]
pub struct ImagePasteAnswer {
    id: ImagePasteId,
    sender: std::sync::mpsc::Sender<(ImagePasteId, ImagePaste)>,
    waker: Option<crate::app::Waker>,
}

impl ImagePasteAnswer {
    pub(crate) fn new(
        id: ImagePasteId,
        sender: std::sync::mpsc::Sender<(ImagePasteId, ImagePaste)>,
        waker: Option<crate::app::Waker>,
    ) -> Self {
        Self { id, sender, waker }
    }

    pub fn id(&self) -> ImagePasteId {
        self.id
    }

    pub fn send(self, paste: ImagePaste) {
        let _ = self.sender.send((self.id, paste));
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }
}
