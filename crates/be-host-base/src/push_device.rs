//! Whether this device shows notifications pushed to it (plans/notifications.md): the
//! state apps watch and the switch they turn. The host that can receive pushes (the web
//! build) installs a `Handler` and keeps the state; every other host leaves it `Unsupported`.

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
    static REVISION: Cell<u64> = const { Cell::new(0) };
}

pub fn install(handler: Handler) {
    HANDLER.with(|slot| *slot.borrow_mut() = Some(handler));
}

pub fn state() -> PushDevice {
    STATE.with(Cell::get)
}

/// How often the state changed, so a host publishes it only when it did.
pub fn revision() -> u64 {
    REVISION.with(Cell::get)
}

pub fn set_state(state: PushDevice) {
    if STATE.with(|held| held.replace(state)) != state {
        REVISION.with(|revision| revision.set(revision.get() + 1));
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
