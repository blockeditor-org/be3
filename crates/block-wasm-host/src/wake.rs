use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicBool, Ordering},
};

use block_gpu_abi as abi;
use wasmtime::{Caller, Linker};

pub(crate) trait Wakes {
    fn wake(&self) -> &Arc<Wake>;
}

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

pub(crate) fn link<T: Wakes + 'static>(linker: &mut Linker<T>) -> Result<(), String> {
    linker
        .func_wrap(abi::HOST_MODULE, abi::HOST_WAKE, |caller: Caller<'_, T>| {
            caller.data().wake().wake();
        })
        .map(|_| ())
        .map_err(|error| format!("{} could not be linked: {error}", abi::HOST_WAKE))
}
