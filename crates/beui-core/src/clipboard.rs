pub use ui_types::clipboard::{PasteKind, Pasted};

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
    pub fn new(
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
