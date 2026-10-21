use serde::{Deserialize, Serialize};

use crate::files::PickedFile;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PasteKind {
    Text,
    Image,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
