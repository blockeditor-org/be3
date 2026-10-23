use geometry::Rect;
use serde::{Deserialize, Serialize};

use crate::clipboard::PasteKind;
use crate::input::{CursorIcon, ImeText, KeyChord, Keyboard};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImeArea {
    pub rect: Rect,
    pub cursor: Rect,
    pub text: Option<ImeText>,
    pub keyboard: Keyboard,
}

impl ImeArea {
    pub fn translated(self, by: geometry::Vec2) -> Self {
        Self {
            rect: self.rect.translate(by),
            cursor: self.cursor.translate(by),
            ..self
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowRequests {
    pub cursor: CursorIcon,
    pub pointer_locked: bool,
    pub ime: Option<ImeArea>,
    pub handles_back: bool,
    pub wants_keyboard: bool,
    pub intercepted_keys: Vec<KeyChord>,
    pub pastes: Vec<PasteKind>,
    pub keyboard_toggles: u32,
}
