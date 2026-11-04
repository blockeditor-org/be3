use beui_types::Pasted;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteRequest {
    Asked,
    Shortcut,
    Typed,
}

pub struct PasteAnswer {
    sender: std::sync::mpsc::Sender<Pasted>,
    waker: Option<crate::program::Waker>,
}

impl PasteAnswer {
    pub fn new(
        sender: std::sync::mpsc::Sender<Pasted>,
        waker: Option<crate::program::Waker>,
    ) -> Self {
        Self { sender, waker }
    }

    pub fn send(self, pasted: Option<Pasted>) {
        if let Some(pasted) = pasted {
            let _ = self.sender.send(pasted);
        }
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }
}

pub struct ClipboardRead(Box<dyn FnOnce(Option<String>)>);

impl ClipboardRead {
    pub fn new(answer: impl FnOnce(Option<String>) + 'static) -> Self {
        Self(Box::new(answer))
    }

    pub fn answer(self, text: Option<String>) {
        (self.0)(text);
    }
}
