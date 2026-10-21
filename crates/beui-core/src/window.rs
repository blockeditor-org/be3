use crate::clipboard::PasteKind;
use crate::color::Color32;
use crate::input::{CursorIcon, ImeArea, KeyChord};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowState {
    pub cursor: CursorIcon,
    pub pointer_locked: bool,
    pub handles_back: bool,
    pub wants_keyboard: bool,
    pub intercepted_keys: Vec<KeyChord>,
    pub paste_kinds: Vec<PasteKind>,
    pub ime: Option<ImeArea>,
    pub typed: u64,
    pub keyboard_toggles: u32,
    pub appearance: Appearance,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum SystemBars {
    Light,
    #[default]
    Dark,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Appearance {
    pub clear_color: Color32,
    pub title: Option<String>,
    pub system_bars: SystemBars,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            clear_color: Color32::BLACK,
            title: None,
            system_bars: SystemBars::default(),
        }
    }
}

impl WindowState {
    pub(crate) fn begin_frame(&mut self, typed: bool) {
        *self = Self {
            pointer_locked: self.pointer_locked,
            appearance: std::mem::take(&mut self.appearance),
            typed: self.typed.wrapping_add(u64::from(typed)),
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

#[derive(Default)]
struct Reported {
    fullscreen: std::cell::Cell<Option<bool>>,
    size: std::cell::Cell<Option<(u32, u32)>>,
    scale: std::cell::Cell<Option<f32>>,
    keyboard: std::cell::Cell<Option<bool>>,
}

#[derive(Clone, Default)]
pub struct WindowReports(std::rc::Rc<Reported>);

impl WindowReports {
    pub fn fullscreen(&self, fullscreen: bool) {
        self.0.fullscreen.set(Some(fullscreen));
    }

    pub fn resized(&self, width: u32, height: u32) {
        self.0.size.set(Some((width, height)));
    }

    pub fn scaled(&self, pixels_per_point: f32) {
        self.0.scale.set(Some(pixels_per_point));
    }

    pub fn keyboard_attached(&self, attached: bool) {
        self.0.keyboard.set(Some(attached));
    }

    pub(crate) fn take_keyboard(&self) -> Option<bool> {
        self.0.keyboard.take()
    }

    pub(crate) fn pending(&self) -> bool {
        self.0.fullscreen.get().is_some()
            || self.0.size.get().is_some()
            || self.0.scale.get().is_some()
            || self.0.keyboard.get().is_some()
    }

    pub(crate) fn take_fullscreen(&self) -> Option<bool> {
        self.0.fullscreen.take()
    }

    pub(crate) fn take_size(&self) -> Option<(u32, u32)> {
        self.0.size.take()
    }

    pub(crate) fn take_scale(&self) -> Option<f32> {
        self.0.scale.take()
    }
}
