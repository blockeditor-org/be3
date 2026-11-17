//! The sandbox in the browser: the same calls as `be_wasm_host::sandbox`, natively, but each
//! instance runs in a web worker of its own (be-wasm-guest-host's `WebGuest`), started from the
//! URL its module is staged at, with be-host's `worker.js` linking it as an app is linked, and no
//! display. The browser compiles no module from bytes for a guest yet: no root guest runs there
//! (plans/host_as_platform_adapter.md, step 4), and apps compile none.

use std::{
    convert::Infallible,
    sync::{Arc, atomic::AtomicBool},
};

use be_wasm_guest_host::{Wake, Worker, web::WebGuest};

/// A module, by the SHA-256 of the bytes it was compiled from.
pub type ModuleId = [u8; 32];

/// A compiled module: none in the browser, which compiles none.
#[derive(Clone)]
pub enum Module {}

impl Module {
    pub fn id(&self) -> ModuleId {
        match *self {}
    }
}

const NO_COMPILES: &str = "this host compiles no modules from bytes";

/// A compile, which in the browser has already failed.
pub struct Compiling;

impl Compiling {
    pub fn done(&self) -> Option<Result<Module, String>> {
        Some(Err(NO_COMPILES.to_owned()))
    }
}

pub fn compile(_bytes: Vec<u8>, _wake: Wake) -> Compiling {
    Compiling
}

/// How an instance runs.
pub struct Config {
    /// Its name, which says which instance could not start.
    pub name: String,
    /// The most bytes its memory may hold, which the browser cannot hold a worker to: there it is
    /// the most its own memory declares.
    #[allow(dead_code)]
    pub memory_limit: usize,
}

/// What an instance runs.
pub enum Source {
    Module(Module),
    App(Staged),
}

/// A staged app's module: the URL it is staged at, relative to the page, such as
/// `workers/be3.map/tiles.wasm`. Its libraries lie at the bundle's root.
#[derive(Clone)]
pub struct Staged {
    url: String,
}

impl Staged {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

/// The script that links a staged app's module in its web worker, beside the page.
const EXTENSION: &str = "./worker.js";

/// An instance in a web worker of its own. Dropping it terminates the worker.
pub struct Instance {
    worker: Worker<WebGuest<Infallible>>,
    /// Why it stopped, once it did.
    failure: Option<String>,
}

/// Starts an instance of `source`, which calls `wake` whenever it has sent frames or stopped.
pub fn spawn(source: Source, config: Config, wake: Wake) -> Result<Instance, String> {
    // A module from bytes cannot be: none compiles.
    let Source::App(staged) = source;
    let waiting = Arc::new(AtomicBool::new(false));
    let guest = WebGuest::start(&staged.url, Some(EXTENSION), wake, Arc::clone(&waiting))
        .map_err(|error| format!("{} could not start: {error}", config.name))?;
    Ok(Instance {
        worker: Worker::new(guest, waiting),
        failure: None,
    })
}

impl Instance {
    /// Queues `frames` for the instance, which is stepped with them as soon as it is free.
    pub fn send(&mut self, frames: Vec<Vec<u8>>) -> Result<(), String> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        self.failure = self.worker.queue(frames);
        self.failure.clone().map_or(Ok(()), Err)
    }

    /// The frames the instance sent since the last call, or, once it sent nothing more, why it
    /// stopped.
    pub fn receive(&mut self) -> Result<Vec<Vec<u8>>, String> {
        if self.failure.is_none() {
            self.failure = self.worker.poll();
        }
        let received = self.worker.take_received();
        match &self.failure {
            Some(failure) if received.is_empty() => Err(failure.clone()),
            _ => Ok(received),
        }
    }
}
