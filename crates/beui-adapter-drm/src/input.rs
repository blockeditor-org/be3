use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use beui::Waker;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DeviceId {
    pub name: String,
    pub vendor: u32,
    pub product: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PointerConfig {
    pub speed: Option<f64>,
    pub tap_to_click: Option<bool>,
    pub natural_scroll: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PointerDevice {
    pub id: DeviceId,
    pub defaults: PointerConfig,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputConfig {
    pub layout: String,
    pub variant: String,
    pub options: String,
    pub repeat_delay: Duration,
    pub repeat_interval: Duration,
    pub pointers: Vec<(DeviceId, PointerConfig)>,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            layout: String::new(),
            variant: String::new(),
            options: String::new(),
            repeat_delay: Duration::from_millis(600),
            repeat_interval: Duration::from_millis(40),
            pointers: Vec::new(),
        }
    }
}

impl InputConfig {
    pub(crate) fn same_keymap(&self, other: &Self) -> bool {
        self.layout == other.layout
            && self.variant == other.variant
            && self.options == other.options
    }

    pub(crate) fn pointer(&self, id: &DeviceId) -> PointerConfig {
        self.pointers
            .iter()
            .find(|(device, _)| device == id)
            .map(|(_, config)| *config)
            .unwrap_or_default()
    }
}

type DevicesListener = Box<dyn Fn(&[PointerDevice])>;

#[derive(Clone)]
pub struct InputControl {
    pending: Rc<RefCell<Option<InputConfig>>>,
    devices: Rc<RefCell<Vec<PointerDevice>>>,
    listener: Rc<RefCell<Option<DevicesListener>>>,
    waker: Waker,
}

impl InputControl {
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            pending: Rc::default(),
            devices: Rc::default(),
            listener: Rc::default(),
            waker,
        }
    }

    pub fn set(&self, config: InputConfig) {
        *self.pending.borrow_mut() = Some(config);
        self.waker.wake();
    }

    pub fn on_devices(&self, listener: impl Fn(&[PointerDevice]) + 'static) {
        listener(&self.devices.borrow());
        *self.listener.borrow_mut() = Some(Box::new(listener));
    }

    pub(crate) fn take(&self) -> Option<InputConfig> {
        self.pending.borrow_mut().take()
    }

    pub(crate) fn set_devices(&self, devices: Vec<PointerDevice>) {
        if *self.devices.borrow() == devices {
            return;
        }
        *self.devices.borrow_mut() = devices;
        if let Some(listener) = self.listener.borrow().as_ref() {
            listener(&self.devices.borrow());
        }
    }
}
