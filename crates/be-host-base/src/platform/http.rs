use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct Aborted {
    flag: Arc<AtomicBool>,
    #[cfg(target_arch = "wasm32")]
    controller: std::rc::Rc<std::cell::RefCell<Option<web_sys::AbortController>>>,
}

impl Aborted {
    pub fn abort(&self) {
        self.flag.store(true, Ordering::Release);
        #[cfg(target_arch = "wasm32")]
        if let Some(controller) = self.controller.borrow_mut().take() {
            controller.abort();
        }
    }

    pub fn aborted(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    pub fn flag(&self) -> AbortFlag {
        AbortFlag(Arc::clone(&self.flag))
    }
}

#[derive(Clone)]
pub struct AbortFlag(Arc<AtomicBool>);

impl AbortFlag {
    pub fn aborted(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// A request an app sends (`send`): any method, its headers in order, and its body, empty
/// for none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// A response's status and headers, whatever the status, and the URL it came from after
/// any redirects, which the caller checks as it checked the request's.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Head {
    pub url: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
}

/// What `send` delivers, in order: `Head` or `Failed` first; after `Head`, the body's chunks
/// and then `Done`, or `Failed` if the rest did not arrive. Nothing comes after an abort.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Head(Head),
    Chunk(Vec<u8>),
    Done,
    Failed(String),
}

/// The most body chunks `send` reads ahead of what its caller passed on (`Flow::release`),
/// so a slow reader holds the download back instead of the host holding the body.
pub const CHUNKS_AHEAD: usize = 4;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
pub use native::{Flow, send};
#[cfg(target_arch = "wasm32")]
pub use web::{Flow, fetch, run as get, send};
