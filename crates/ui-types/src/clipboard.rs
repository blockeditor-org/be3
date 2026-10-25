pub use crate::types::{PasteKind, Pasted};

impl Pasted {
    pub fn kind(&self) -> PasteKind {
        match self {
            Pasted::Text(_) => PasteKind::Text,
            Pasted::Image(_) => PasteKind::Image,
        }
    }
}
