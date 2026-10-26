pub(crate) mod hosted;

use std::sync::atomic::{AtomicU64, Ordering};

use crate::color::Color32;
use crate::geometry::{Pos2, Vec2};

pub use ui_types::window::WindowRequests;

pub const MAX_PIXELS: f32 = 8192.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowSize {
    pub size: Vec2,
    pub scale: f32,
}

impl WindowSize {
    pub fn parse(text: &str) -> Option<Self> {
        let (size, scale) = match text.split_once('@') {
            Some((size, scale)) => (
                size,
                scale
                    .parse::<f32>()
                    .ok()
                    .filter(|scale| scale.is_finite() && *scale > 0.0)?,
            ),
            None => (text, 1.0),
        };
        let (width, height) = size.split_once('x')?;
        let size = Vec2::new(width.parse().ok()?, height.parse().ok()?);
        Some(Self { size, scale }).filter(Self::fits)
    }

    pub fn fits(&self) -> bool {
        let fits = |length: f32| {
            (1.0..=MAX_PIXELS).contains(&length)
                && (1.0..=MAX_PIXELS).contains(&(length * self.scale))
        };
        fits(self.size.x) && fits(self.size.y)
    }
}

pub enum Resize {
    Window(WindowSize),
    Screen(Vec2),
}

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
    keyboard: std::cell::Cell<Option<bool>>,
    touchscreen: std::cell::Cell<Option<bool>>,
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

    pub fn keyboard_attached(&self, attached: bool) {
        self.0.keyboard.set(Some(attached));
    }

    pub fn touchscreen_attached(&self, attached: bool) {
        self.0.touchscreen.set(Some(attached));
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

    pub(crate) fn take_touchscreen(&self) -> Option<bool> {
        self.0.touchscreen.take()
    }

    pub(crate) fn pending(&self) -> bool {
        self.0.fullscreen.get().is_some()
            || self.0.size.get().is_some()
            || self.0.scale.get().is_some()
            || self.0.keyboard.get().is_some()
            || self.0.touchscreen.get().is_some()
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
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct WindowId(u64);

static NEXT_WINDOW: AtomicU64 = AtomicU64::new(1);

impl WindowId {
    pub const MAIN: Self = Self(0);

    pub fn next() -> Self {
        Self(NEXT_WINDOW.fetch_add(1, Ordering::Relaxed))
    }

    pub fn is_main(self) -> bool {
        self == Self::MAIN
    }

    pub fn raw(self) -> u64 {
        self.0
    }
}

impl Default for WindowId {
    fn default() -> Self {
        Self::MAIN
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct HostPlacement(pub String);

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WindowHosting {
    #[default]
    None,
    Placed,
    Any,
}

#[derive(Clone, PartialEq, Debug)]
pub enum WindowPlacement {
    AtPointer,
    Over { window: WindowId, at: Pos2 },
    Host(HostPlacement),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum WindowEdge {
    Top,
    Bottom,
    Left,
    Right,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Clone, PartialEq, Debug)]
pub enum WindowRequest {
    Open {
        window: WindowId,
        placement: WindowPlacement,
        size: Vec2,
    },
    BeginMove(WindowId),
    BeginResize(WindowId, WindowEdge),
    Resize {
        window: WindowId,
        size: Vec2,
    },
    Close(WindowId),
}

impl WindowRequest {
    pub fn window(&self) -> WindowId {
        match self {
            Self::Open { window, .. } | Self::Resize { window, .. } => *window,
            Self::BeginMove(window) | Self::BeginResize(window, _) | Self::Close(window) => *window,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum WindowReport {
    Resized {
        window: WindowId,
        size: Vec2,
    },
    Placed {
        window: WindowId,
        placement: HostPlacement,
    },
    Scaled {
        window: WindowId,
        pixels_per_point: f32,
    },
    Moved {
        window: WindowId,
        origin: Pos2,
    },
    Closed(WindowId),
    Focused {
        window: WindowId,
        focused: bool,
    },
}

impl WindowReport {
    pub fn window(&self) -> WindowId {
        match self {
            Self::Resized { window, .. }
            | Self::Placed { window, .. }
            | Self::Scaled { window, .. }
            | Self::Moved { window, .. }
            | Self::Focused { window, .. } => *window,
            Self::Closed(window) => *window,
        }
    }
}
