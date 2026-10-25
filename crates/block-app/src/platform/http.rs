use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(crate) const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Default)]
pub(crate) struct Aborted {
    flag: Arc<AtomicBool>,
    #[cfg(target_arch = "wasm32")]
    controller: std::rc::Rc<std::cell::RefCell<Option<web_sys::AbortController>>>,
}

impl Aborted {
    pub(crate) fn abort(&self) {
        self.flag.store(true, Ordering::Release);
        #[cfg(target_arch = "wasm32")]
        if let Some(controller) = self.controller.borrow_mut().take() {
            controller.abort();
        }
    }

    pub(crate) fn aborted(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    pub(crate) fn flag(&self) -> AbortFlag {
        AbortFlag(Arc::clone(&self.flag))
    }
}

#[derive(Clone)]
pub(crate) struct AbortFlag(Arc<AtomicBool>);

impl AbortFlag {
    pub(crate) fn aborted(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::fetch;
#[cfg(target_arch = "wasm32")]
pub(crate) use web::{fetch, run as get};
