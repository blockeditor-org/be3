use crate::clipboard::PasteKind;
use crate::input::{CursorIcon, ImeArea, KeyChord};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowState {
    pub cursor: CursorIcon,
    pub touch_emulation: bool,
    pub pointer_locked: bool,
    pub handles_back: bool,
    pub wants_keyboard: bool,
    pub intercepted_keys: Vec<KeyChord>,
    pub paste_kinds: Vec<PasteKind>,
    pub ime: Option<ImeArea>,
}

impl WindowState {
    pub(crate) fn begin_frame(&mut self) {
        *self = Self {
            pointer_locked: self.pointer_locked,
            touch_emulation: self.touch_emulation,
            ..Self::default()
        };
    }

    pub fn scaled(mut self, scale: f32) -> Self {
        if let Some(area) = &mut self.ime {
            area.rect = area.rect.scaled(scale);
            area.cursor = area.cursor.scaled(scale);
        }
        self
    }
}
