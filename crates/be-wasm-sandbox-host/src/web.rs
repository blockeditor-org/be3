//! The sandbox in the browser: the same calls as `be_wasm_host::sandbox`, natively, but each
//! instance runs in a web worker of its own (be-wasm-guest-host's `WebGuest`). A staged app's
//! module is started from the URL it is staged at, with be-host's `worker.js` linking it as an
//! app is linked, and no display. A module from bytes is compiled on the page with
//! `WebAssembly.compile`, which never blocks it, once per page for the same bytes, and each
//! instance of it is a plain worker the page posts the compiled `WebAssembly.Module` to, which
//! answers only its transport (plans/host_as_platform_adapter.md, The sandbox API).

use std::{
    cell::RefCell,
    collections::HashMap,
    convert::Infallible,
    rc::{Rc, Weak},
    sync::{Arc, atomic::AtomicBool},
};

use be_wasm_guest_host::{Wake, Worker, web::WebGuest};
use js_sys::{Uint8Array, WebAssembly};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// A module, by the SHA-256 of the bytes it was compiled from.
pub type ModuleId = [u8; 32];

/// A compiled pure guest's module, which any number of instances run without compiling it again.
#[derive(Clone)]
pub struct Module {
    id: ModuleId,
    compiled: Rc<WebAssembly::Module>,
}

impl Module {
    pub fn id(&self) -> ModuleId {
        self.id
    }
}

thread_local! {
    /// Every module compiled and still held, so the same bytes compile once. Only the page
    /// compiles, on its one thread.
    static COMPILED: RefCell<HashMap<ModuleId, Weak<WebAssembly::Module>>> =
        RefCell::new(HashMap::new());
}

/// The only imports a pure guest may have: its transport (be-wasm-guest-abi), which the worker
/// answers itself.
const TRANSPORT: [&str; 4] = ["host_send", "host_receive", "host_wake", "host_panic"];

/// A compile under way, which the browser finishes off the page's thread.
pub struct Compiling {
    done: Rc<RefCell<Option<Result<Module, String>>>>,
}

impl Compiling {
    /// The module, or why the bytes are not a pure guest's module, once compiled.
    pub fn done(&self) -> Option<Result<Module, String>> {
        self.done.borrow().clone()
    }
}

/// Compiles a pure guest's module (guides/app_libraries.md, Pure guests) from `bytes` with
/// `WebAssembly.compile`, or finds it compiled already, and calls `wake` once it is done.
pub fn compile(bytes: Vec<u8>, wake: Wake) -> Compiling {
    let id = be_wasm_dylink::sha256(&bytes);
    let done = Rc::new(RefCell::new(None));
    if let Some(compiled) = compiled(id) {
        *done.borrow_mut() = Some(Ok(Module { id, compiled }));
        wake();
        return Compiling { done };
    }
    let finishing = Rc::clone(&done);
    wasm_bindgen_futures::spawn_local(async move {
        let bytes = Uint8Array::from(bytes.as_slice());
        let compiled = JsFuture::from(WebAssembly::compile(&bytes)).await;
        *finishing.borrow_mut() = Some(finish(id, compiled));
        wake();
    });
    Compiling { done }
}

fn compiled(id: ModuleId) -> Option<Rc<WebAssembly::Module>> {
    COMPILED.with(|compiled| compiled.borrow().get(&id)?.upgrade())
}

fn finish(
    id: ModuleId,
    compiled: Result<wasm_bindgen::JsValue, wasm_bindgen::JsValue>,
) -> Result<Module, String> {
    let compiled: WebAssembly::Module = compiled
        .map_err(|error| format!("the module is not valid: {}", describe(&error)))?
        .unchecked_into();
    pure(&compiled)?;
    let compiled = COMPILED.with(|held| {
        let mut held = held.borrow_mut();
        held.retain(|_, module| module.strong_count() > 0);
        // Two compiles of the same bytes may have raced: the first one kept is the one shared.
        match held.get(&id).and_then(Weak::upgrade) {
            Some(kept) => kept,
            None => {
                let compiled = Rc::new(compiled);
                held.insert(id, Rc::downgrade(&compiled));
                compiled
            }
        }
    });
    Ok(Module { id, compiled })
}

/// Checks that `module` imports nothing but its transport, as a pure guest's module does.
fn pure(module: &WebAssembly::Module) -> Result<(), String> {
    for wanted in WebAssembly::Module::imports(module).iter() {
        let field = |key: &str| {
            js_sys::Reflect::get(&wanted, &key.into())
                .ok()
                .and_then(|value| value.as_string())
                .unwrap_or_default()
        };
        let (from, name) = (field("module"), field("name"));
        if from != "be3_host" || !TRANSPORT.contains(&name.as_str()) {
            return Err(format!(
                "the guest imports {from}.{name}, which a pure guest may not"
            ));
        }
    }
    Ok(())
}

fn describe(error: &wasm_bindgen::JsValue) -> String {
    error
        .dyn_ref::<js_sys::Error>()
        .map(|error| String::from(error.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "the browser refused it".to_owned())
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
    let waiting = Arc::new(AtomicBool::new(false));
    let guest = match source {
        Source::Module(module) => {
            WebGuest::start_module(&module.compiled, wake, Arc::clone(&waiting))
        }
        Source::App(staged) => {
            WebGuest::start(&staged.url, Some(EXTENSION), wake, Arc::clone(&waiting))
        }
    }
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
