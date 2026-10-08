use std::cell::RefCell;
use std::rc::Rc;

use beui::Waker;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_millihertz: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Monitor {
    pub id: String,
    pub name: String,
    pub connector: String,
    pub modes: Vec<DisplayMode>,
    pub default: DisplayMode,
    pub preferred: DisplayMode,
    pub current: DisplayMode,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DisplayConfig {
    pub modes: Vec<(String, DisplayMode)>,
}

impl DisplayConfig {
    pub(crate) fn mode(&self, monitor: &str) -> Option<DisplayMode> {
        self.modes
            .iter()
            .find(|(id, _)| id == monitor)
            .map(|(_, mode)| *mode)
    }
}

type MonitorsListener = Rc<dyn Fn(&[Monitor])>;

#[derive(Clone)]
pub struct DisplayControl {
    pending: Rc<RefCell<Option<DisplayConfig>>>,
    monitors: Rc<RefCell<Vec<Monitor>>>,
    listener: Rc<RefCell<Option<MonitorsListener>>>,
    waker: Waker,
}

impl DisplayControl {
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            pending: Rc::default(),
            monitors: Rc::default(),
            listener: Rc::default(),
            waker,
        }
    }

    pub fn set(&self, config: DisplayConfig) {
        *self.pending.borrow_mut() = Some(config);
        self.waker.wake();
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
