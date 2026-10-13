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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhysicalSize {
    pub width: u32,
    pub height: u32,
    pub pixels_per_point: f32,
}

#[derive(Default)]
struct Reported {
    fullscreen: std::cell::Cell<Option<bool>>,
    size: std::cell::Cell<Option<PhysicalSize>>,
}

#[derive(Clone, Default)]
pub struct WindowReports(std::rc::Rc<Reported>);

impl WindowReports {
    pub fn fullscreen(&self, fullscreen: bool) {
        self.0.fullscreen.set(Some(fullscreen));
    }

    pub fn resized(&self, size: PhysicalSize) {
        self.0.size.set(Some(size));
    }

    pub(crate) fn pending(&self) -> bool {
        self.0.fullscreen.get().is_some() || self.0.size.get().is_some()
    }

    pub(crate) fn take_fullscreen(&self) -> Option<bool> {
        self.0.fullscreen.take()
    }

    pub(crate) fn take_size(&self) -> Option<PhysicalSize> {
        self.0.size.take()
    }
}
