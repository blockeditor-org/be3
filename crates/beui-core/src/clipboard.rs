use crate::file_picker::PickedFile;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PasteKind {
    Text,
    Image,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pasted {
    Text(String),
    Image(PickedFile),
}

impl Pasted {
    pub fn kind(&self) -> PasteKind {
        match self {
            Pasted::Text(_) => PasteKind::Text,
            Pasted::Image(_) => PasteKind::Image,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteRequest {
    Asked,
    Shortcut,
    Typed,
}

pub struct PasteAnswer {
    sender: std::sync::mpsc::Sender<Pasted>,
    waker: Option<crate::app::Waker>,
}

impl PasteAnswer {
    pub(crate) fn new(
        sender: std::sync::mpsc::Sender<Pasted>,
        waker: Option<crate::app::Waker>,
    ) -> Self {
        Self { sender, waker }
    }

    pub fn send(self, pasted: Option<Pasted>) {
        let Some(pasted) = pasted else {
            return;
        };
        let _ = self.sender.send(pasted);
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }
}
