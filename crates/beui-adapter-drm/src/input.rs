use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use beui::Waker;

#[derive(Clone, Debug, PartialEq)]
pub struct InputConfig {
    pub layout: String,
    pub variant: String,
    pub options: String,
    pub repeat_delay: Duration,
    pub repeat_interval: Duration,
    pub pointer_speed: f64,
    pub tap_to_click: Option<bool>,
    pub natural_scroll: bool,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            layout: String::new(),
            variant: String::new(),
            options: String::new(),
            repeat_delay: Duration::from_millis(600),
            repeat_interval: Duration::from_millis(40),
            pointer_speed: 0.0,
            tap_to_click: None,
            natural_scroll: false,
        }
    }
}

impl InputConfig {
    pub(crate) fn same_keymap(&self, other: &Self) -> bool {
        self.layout == other.layout
            && self.variant == other.variant
            && self.options == other.options
    }
}

#[derive(Clone)]
pub struct InputControl {
    pending: Rc<RefCell<Option<InputConfig>>>,
    waker: Waker,
}

impl InputControl {
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            pending: Rc::default(),
            waker,
        }
    }

    pub fn set(&self, config: InputConfig) {
        *self.pending.borrow_mut() = Some(config);
        self.waker.wake();
    }

    pub(crate) fn take(&self) -> Option<InputConfig> {
        self.pending.borrow_mut().take()
    }
}
