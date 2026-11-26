//! Whether this device shows notifications pushed to it (guides/blocks/server.md,
//! "Notifications"): the state apps watch and the switch they turn. A host that can receive
//! pushes (the web build, and Android through UnifiedPush) installs a `Handler` and keeps the
//! state; every other host leaves it `Unsupported`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PushDevice {
    #[default]
    Unsupported,
    Off,
    On,
    /// The person refused the permission to show notifications.
    Denied,
}

/// Turns pushes on or off and answers with the browser's refusal as text.
pub type Handler = Rc<dyn Fn(bool, Box<dyn FnOnce(Result<(), String>)>)>;

thread_local! {
    static HANDLER: RefCell<Option<Handler>> = const { RefCell::new(None) };
    static STATE: Cell<PushDevice> = const { Cell::new(PushDevice::Unsupported) };
}

pub fn install(handler: Handler) {
    HANDLER.with(|slot| *slot.borrow_mut() = Some(handler));
}

pub fn state() -> PushDevice {
    STATE.with(Cell::get)
}

/// Publishes the state to the apps that watch it (`PushDeviceWatch`): once as the host starts,
/// and again whenever `set_state` changes it.
pub fn publish() {
    crate::watches::publish::<be_sdk_api::PushDeviceWatch>(&match state() {
        PushDevice::Unsupported => None,
        PushDevice::Off => Some(be_sdk_api::PushDevice::Off),
        PushDevice::On => Some(be_sdk_api::PushDevice::On),
        PushDevice::Denied => Some(be_sdk_api::PushDevice::Denied),
    });
}

pub fn set_state(state: PushDevice) {
    if STATE.with(|held| held.replace(state)) != state {
        publish();
        crate::host::wake();
    }
}

/// Turns pushes on or off; `reply` hears whether it worked.
pub fn set(on: bool, reply: Box<dyn FnOnce(Result<(), String>)>) {
    match HANDLER.with(|slot| slot.borrow().clone()) {
        Some(handler) => handler(on, reply),
        None => reply(Err("This device cannot show notifications.".to_owned())),
    }
}

#[cfg(test)]
mod tests;
