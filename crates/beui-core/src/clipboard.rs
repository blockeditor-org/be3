use beui_types::ClipItem;
pub use beui_types::clip::{Clip, ClipData, ClipEntry, HeldClip, Render, RenderAnswer};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteRequest {
    Asked,
    /// Ctrl+V, or Ctrl+Shift+V when `plain`: plain text only.
    Shortcut {
        plain: bool,
    },
    Typed,
}

pub struct PasteAnswer {
    sender: std::sync::mpsc::Sender<ClipItem>,
    waker: Option<crate::program::Waker>,
}

impl PasteAnswer {
    pub fn new(
        sender: std::sync::mpsc::Sender<ClipItem>,
        waker: Option<crate::program::Waker>,
    ) -> Self {
        Self { sender, waker }
    }

    pub fn send(self, pasted: Option<ClipItem>) {
        if let Some(pasted) = pasted {
            let _ = self.sender.send(pasted);
        }
        if let Some(waker) = &self.waker {
            waker.wake();
        }
    }
}

/// A read of the clipboard by the program itself (`Context::read_clipboard`).
pub struct ClipboardRead {
    pub formats: Vec<String>,
    answer: Box<dyn FnOnce(Option<ClipItem>)>,
}

impl ClipboardRead {
    pub fn new(formats: Vec<String>, answer: impl FnOnce(Option<ClipItem>) + 'static) -> Self {
        Self {
            formats,
            answer: Box::new(answer),
        }
    }

    pub fn answer(self, item: Option<ClipItem>) {
        (self.answer)(item);
    }
}
