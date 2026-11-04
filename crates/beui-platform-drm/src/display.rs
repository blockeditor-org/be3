use std::cell::{Cell, RefCell};
use std::rc::Rc;

use beui::Pos2;
use beui::native::Waker;

use crate::rotation::Rotation;

pub use be_lib_ui_types::DisplayMode;

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub connector: String,
    pub modes: Vec<DisplayMode>,
    pub default: DisplayMode,
    pub preferred: DisplayMode,
    pub current: DisplayMode,
    pub rotation: Rotation,
    pub default_scale: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// A display mirroring one that is not on the desktop is on the desktop itself;
/// with none left on the desktop the main one is, so a session always has one.
/// `Off` darkens the display but keeps its connector and CRTC.
pub enum DisplayUse {
    #[default]
    Desktop,
    Mirror(String),
    Off,
}

#[derive(Clone, Debug, Default, PartialEq)]
/// How the displays are arranged (`plan.rs`, then `layout::arrange`). A display's
/// scale comes from `scales`, else `BEUI_SCALE`, else `density.rs`'s guess from
/// its physical size. `places` are kept while a display shares an edge with one
/// placed before it and overlaps none, else moved to the nearest place that does;
/// an unplaced display goes right of the rest.
pub struct DisplayConfig {
    pub modes: Vec<(String, DisplayMode)>,
    pub rotations: Vec<(String, Rotation)>,
    pub scales: Vec<(String, f32)>,
    pub uses: Vec<(String, DisplayUse)>,
    pub main: Option<String>,
    pub places: Vec<(String, Pos2)>,
}

impl DisplayConfig {
    pub(crate) fn used(&self, monitor: &str) -> DisplayUse {
        self.uses
            .iter()
            .find(|(id, _)| id == monitor)
            .map(|(_, used)| used.clone())
            .unwrap_or_default()
    }

    pub(crate) fn place(&self, monitor: &str) -> Option<Pos2> {
        self.places
            .iter()
            .find(|(id, place)| id == monitor && place.x.is_finite() && place.y.is_finite())
            .map(|(_, place)| *place)
    }

    pub(crate) fn same_arrangement(&self, other: &Self) -> bool {
        self.uses == other.uses && self.main == other.main && self.places == other.places
    }

    pub(crate) fn mode(&self, monitor: &str) -> Option<DisplayMode> {
        self.modes
            .iter()
            .find(|(id, _)| id == monitor)
            .map(|(_, mode)| *mode)
    }

    pub(crate) fn scale(&self, monitor: &str) -> Option<f32> {
        self.scales
            .iter()
            .find(|(id, scale)| id == monitor && scale.is_finite() && *scale > 0.0)
            .map(|(_, scale)| *scale)
    }

    pub(crate) fn rotation(&self, monitor: &str) -> Rotation {
        self.rotations
            .iter()
            .find(|(id, _)| id == monitor)
            .map(|(_, rotation)| *rotation)
            .unwrap_or_default()
    }
}

type MonitorsListener = Rc<dyn Fn(&[Monitor])>;

#[derive(Clone)]
pub struct DisplayControl {
    pending: Rc<RefCell<Option<DisplayConfig>>>,
    blanked: Rc<Cell<Option<bool>>>,
    woken: Rc<Cell<bool>>,
    monitors: Rc<RefCell<Vec<Monitor>>>,
    listener: Rc<RefCell<Option<MonitorsListener>>>,
    waker: Waker,
}

impl DisplayControl {
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            pending: Rc::default(),
            blanked: Rc::default(),
            woken: Rc::default(),
            monitors: Rc::default(),
            listener: Rc::default(),
            waker,
        }
    }

    pub fn set(&self, config: DisplayConfig) {
        *self.pending.borrow_mut() = Some(config);
        self.waker.wake();
    }

    pub fn set_blanked(&self, blanked: bool) {
        self.blanked.set(Some(blanked));
        self.waker.wake();
    }

    pub(crate) fn take_blanked(&self) -> Option<bool> {
        self.blanked.take()
    }

    pub fn take_woken(&self) -> bool {
        self.woken.take()
    }

    pub(crate) fn woke(&self) {
        self.woken.set(true);
    }

    pub fn on_monitors(&self, listener: impl Fn(&[Monitor]) + 'static) {
        let monitors = self.monitors.borrow().clone();
        listener(&monitors);
        *self.listener.borrow_mut() = Some(Rc::new(listener));
    }

    pub(crate) fn take(&self) -> Option<DisplayConfig> {
        self.pending.borrow_mut().take()
    }

    pub(crate) fn set_monitors(&self, monitors: Vec<Monitor>) {
        if *self.monitors.borrow() == monitors {
            return;
        }
        *self.monitors.borrow_mut() = monitors.clone();
        let listener = self.listener.borrow().clone();
        if let Some(listener) = listener {
            listener(&monitors);
        }
    }
}
