use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

use be_wasm_guest_abi as abi;

use crate::engine::Defined;
use crate::state::state_of;

#[derive(Default)]
pub(crate) struct Wake {
    woken: AtomicBool,
    notify: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl Wake {
    pub(crate) fn wake(&self) {
        self.woken.store(true, Ordering::Release);
        if let Some(notify) = self.notify.get() {
            notify();
        }
    }

    pub(crate) fn take(&self) -> bool {
        self.woken.swap(false, Ordering::AcqRel)
    }

    pub(crate) fn notify(&self, notify: impl Fn() + Send + Sync + 'static) {
        let _ = self.notify.set(Box::new(notify));
    }
}

fn host_wake(
    _: &mut v8::PinScope,
    arguments: v8::FunctionCallbackArguments,
    _: v8::ReturnValue<v8::Value>,
) {
    // unsafe(ffi): the function's data is its State (state_of).
    let state = unsafe { state_of(arguments.data()) };
    let wake = Arc::clone(&state.wake);
    if let Err(panic) = crate::gpu::contained(|| wake.wake()) {
        state.fatal.get_or_insert(panic);
        state.isolate.terminate_execution();
    }
}

pub(crate) fn define<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    defined: &mut Defined<'s>,
    data: v8::Local<'s, v8::Value>,
) -> Result<(), String> {
    crate::engine::function(scope, defined, abi::HOST_MODULE, abi::HOST_WAKE, data, host_wake)
}
