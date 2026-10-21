use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use beui::Waker;

pub use ui_types::{InputDevice, PointerSettings, PointerSpeed};

#[derive(Clone, Debug, PartialEq)]
pub struct PointerDevice {
    pub id: InputDevice,
    pub defaults: PointerSettings,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InputConfig {
    pub layout: String,
    pub variant: String,
    pub options: String,
    pub repeat_delay: Duration,
    pub repeat_interval: Duration,
    pub every_pointer: PointerSettings,
    pub pointers: Vec<(InputDevice, PointerSettings)>,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            layout: String::new(),
            variant: String::new(),
            options: String::new(),
            repeat_delay: Duration::from_millis(600),
            repeat_interval: Duration::from_millis(40),
            every_pointer: PointerSettings::default(),
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

    pub(crate) fn pointer(&self, id: &InputDevice) -> PointerSettings {
        self.pointers
            .iter()
            .find(|(device, _)| device == id)
            .map(|(_, config)| *config)
            .unwrap_or_default()
            .or(self.every_pointer)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RawInput {
    Key { code: u32, pressed: bool },
    Button { code: u32, pressed: bool },
    Wheel { x: i32, y: i32 },
}

pub trait InputFilter {
    fn input(&mut self, input: RawInput, now: Duration) -> Vec<RawInput>;

    fn timer(&mut self, now: Duration) -> Vec<RawInput>;

    fn deadline(&self) -> Option<Duration>;

    fn forget_held(&mut self);
}

pub type SharedFilter = Rc<RefCell<dyn InputFilter>>;

type DevicesListener = Rc<dyn Fn(&[PointerDevice])>;

#[derive(Clone)]
pub struct InputControl {
    pending: Rc<RefCell<Option<InputConfig>>>,
    devices: Rc<RefCell<Vec<PointerDevice>>>,
    listener: Rc<RefCell<Option<DevicesListener>>>,
    filter: Rc<RefCell<Option<SharedFilter>>>,
    injected: Rc<RefCell<Vec<RawInput>>>,
    waker: Waker,
}

impl InputControl {
    pub(crate) fn new(waker: Waker) -> Self {
        Self {
            pending: Rc::default(),
            devices: Rc::default(),
            listener: Rc::default(),
            filter: Rc::default(),
            injected: Rc::default(),
            waker,
        }
    }

    pub fn set_filter(&self, filter: Option<SharedFilter>) {
        *self.filter.borrow_mut() = filter;
        self.waker.wake();
    }

    pub fn inject(&self, inputs: Vec<RawInput>) {
        if inputs.is_empty() {
            return;
        }
        self.injected.borrow_mut().extend(inputs);
        self.waker.wake();
    }

    pub(crate) fn filter(&self) -> Option<SharedFilter> {
        self.filter.borrow().clone()
    }

    pub(crate) fn take_injected(&self) -> Vec<RawInput> {
        std::mem::take(&mut *self.injected.borrow_mut())
    }

    pub fn set(&self, config: InputConfig) {
        *self.pending.borrow_mut() = Some(config);
        self.waker.wake();
    }

    pub fn on_devices(&self, listener: impl Fn(&[PointerDevice]) + 'static) {
        listener(&self.devices.borrow());
        *self.listener.borrow_mut() = Some(Rc::new(listener));
    }

    pub(crate) fn take(&self) -> Option<InputConfig> {
        self.pending.borrow_mut().take()
    }

    pub(crate) fn set_devices(&self, devices: Vec<PointerDevice>) {
        if *self.devices.borrow() == devices {
            return;
        }
        *self.devices.borrow_mut() = devices.clone();
        let listener = self.listener.borrow().clone();
        if let Some(listener) = listener {
            listener(&devices);
        }
    }
}
