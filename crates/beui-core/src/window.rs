pub(crate) mod hosted;

use crate::color::Color32;

pub use ui_types::window::{
    HostPlacement, WindowEdge, WindowId, WindowPlacement, WindowReport, WindowRequest,
    WindowRequests,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WindowState {
    pub requests: WindowRequests,
    pub typed: u64,
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
            requests: WindowRequests {
                pointer_locked: self.requests.pointer_locked,
                ..WindowRequests::default()
            },
            appearance: std::mem::take(&mut self.appearance),
            typed: self.typed.wrapping_add(u64::from(typed)),
        };
    }

    pub fn scaled(mut self, scale: f32) -> Self {
        if let Some(area) = &mut self.requests.ime {
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
    raster: std::cell::Cell<Option<f32>>,
    keyboard: std::cell::Cell<Option<bool>>,
    preferences: std::cell::Cell<Option<crate::preferences::PreferenceLayer>>,
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

    pub fn rastered(&self, pixels_per_point: f32) {
        self.0.raster.set(Some(pixels_per_point));
    }

    pub fn keyboard_attached(&self, attached: bool) {
        self.0.keyboard.set(Some(attached));
    }

    pub fn system_preferences(&self, preferences: crate::preferences::PreferenceLayer) {
        self.0.preferences.set(Some(preferences));
    }

    pub(crate) fn take_system_preferences(&self) -> Option<crate::preferences::PreferenceLayer> {
        self.0.preferences.take()
    }

    pub(crate) fn take_keyboard(&self) -> Option<bool> {
        self.0.keyboard.take()
    }

    pub(crate) fn pending(&self) -> bool {
        self.0.fullscreen.get().is_some()
            || self.0.size.get().is_some()
            || self.0.scale.get().is_some()
            || self.0.raster.get().is_some()
            || self.0.keyboard.get().is_some()
            || self.0.preferences.get().is_some()
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

    pub(crate) fn take_raster(&self) -> Option<f32> {
        self.0.raster.take()
    }
}
