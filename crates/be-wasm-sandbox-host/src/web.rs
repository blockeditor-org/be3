//! The sandbox in the browser: the same calls as `be_wasm_host::sandbox`, natively, but each
//! instance runs in a web worker of its own (be-wasm-guest-host's `WebGuest`). A staged app's
//! module is started from the URL it is staged at, with be-host's `worker.js` linking it as an
//! app is linked, and no display. A module from bytes is compiled on the page with
//! `WebAssembly.compile`, which never blocks it, once per page for the same bytes, with the
//! memory it defines imported instead, as natively; each instance of it is a plain worker the
//! page posts the compiled `WebAssembly.Module` to, which answers only its transport and makes
//! its memory, capped at the instance's limit (plans/host_as_platform_adapter.md, The sandbox
//! API).

use std::{
    cell::RefCell,
    collections::HashMap,
    convert::Infallible,
    rc::{Rc, Weak},
    sync::{Arc, atomic::AtomicBool},
};

use be_wasm_dylink::DefinedMemory;
use be_wasm_guest_host::{Wake, Worker, web::WebGuest};
use js_sys::{Reflect, Uint8Array, WebAssembly};
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

/// A module, by the SHA-256 of the bytes it was compiled from.
pub type ModuleId = [u8; 32];

/// A compiled pure guest's module, which any number of instances run without compiling it again.
#[derive(Clone)]
pub struct Module {
    id: ModuleId,
    compiled: Rc<Compiled>,
}

/// A module as the page compiled it: with the memory it defines imported instead, so each
/// instance is given one capped at its own limit.
struct Compiled {
    module: WebAssembly::Module,
    memory: Option<DefinedMemory>,
}

impl Module {
    pub fn id(&self) -> ModuleId {
        self.id
    }
}

thread_local! {
    /// Every module compiled and still held, so the same bytes compile once. Only the page
    /// compiles, on its one thread.
    static COMPILED: RefCell<HashMap<ModuleId, Weak<Compiled>>> = RefCell::new(HashMap::new());
}

/// The only functions a pure guest may import beside its WASI (be_wasm_pure_wasi::ANSWERED): its
/// transport (be-wasm-guest-abi). The worker answers both itself. Its memory, which compiling
/// imports, is `env.memory`.
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
/// `WebAssembly.compile`, or finds it compiled already by its SHA-256, which `crypto.subtle`
/// takes, and calls `wake` once it is done.
pub fn compile(bytes: Vec<u8>, wake: Wake) -> Compiling {
    let done = Rc::new(RefCell::new(None));
    let finishing = Rc::clone(&done);
    wasm_bindgen_futures::spawn_local(async move {
        let compiled = compiled(bytes).await;
        *finishing.borrow_mut() = Some(compiled);
        wake();
    });
    Compiling { done }
}

async fn compiled(bytes: Vec<u8>) -> Result<Module, String> {
    let bytes = Uint8Array::from(bytes.as_slice());
    let id = sha256(&bytes).await?;
    let kept = COMPILED.with(|held| held.borrow().get(&id).and_then(Weak::upgrade));
    if let Some(compiled) = kept {
        return Ok(Module { id, compiled });
    }
    let (bytes, memory) = be_wasm_dylink::importing_memory(&bytes.to_vec())?;
    let module: WebAssembly::Module =
        JsFuture::from(WebAssembly::compile(&Uint8Array::from(bytes.as_slice())))
            .await
            .map_err(|error| format!("the module is not valid: {}", describe(&error)))?
            .unchecked_into();
    pure(&module)?;
    let compiled = COMPILED.with(|held| {
        let mut held = held.borrow_mut();
        held.retain(|_, module| module.strong_count() > 0);
        // Two compiles of the same bytes may have raced: the first one kept is the one shared.
        match held.get(&id).and_then(Weak::upgrade) {
            Some(kept) => kept,
            None => {
                let compiled = Rc::new(Compiled { module, memory });
                held.insert(id, Rc::downgrade(&compiled));
                compiled
            }
        }
    });
    Ok(Module { id, compiled })
}

/// The SHA-256 of `bytes`, from `crypto.subtle`, which hashes off the page's thread.
async fn sha256(bytes: &Uint8Array) -> Result<ModuleId, String> {
    let unavailable = |_| "this page cannot hash: it has no crypto.subtle".to_owned();
    let subtle = Reflect::get(&js_sys::global(), &"crypto".into())
        .and_then(|crypto| Reflect::get(&crypto, &"subtle".into()))
        .map_err(unavailable)?;
    let digest: js_sys::Function = Reflect::get(&subtle, &"digest".into())
        .map_err(unavailable)?
        .dyn_into()
        .map_err(|_| "this page cannot hash: it has no crypto.subtle".to_owned())?;
    let promise: js_sys::Promise = digest
        .call2(&subtle, &"SHA-256".into(), bytes)
        .map_err(|error| format!("the module could not be hashed: {}", describe(&error)))?
        .unchecked_into();
    let hashed = JsFuture::from(promise)
        .await
        .map_err(|error| format!("the module could not be hashed: {}", describe(&error)))?;
    Uint8Array::new(&hashed)
        .to_vec()
        .try_into()
        .map_err(|_| "the module's hash is not 32 bytes".to_owned())
}

/// Checks that `module` imports nothing but its transport, its WASI and its memory, as a pure
/// guest's module does once its memory is imported.
fn pure(module: &WebAssembly::Module) -> Result<(), String> {
    for wanted in WebAssembly::Module::imports(module).iter() {
        let field = |key: &str| {
            Reflect::get(&wanted, &key.into())
                .ok()
                .and_then(|value| value.as_string())
                .unwrap_or_default()
        };
        let (from, name, kind) = (field("module"), field("name"), field("kind"));
        let transport = kind == "function"
            && ((from == "be3_host" && TRANSPORT.contains(&name.as_str()))
                || be_wasm_pure_wasi::answers(&from, &name));
        let memory =
            from == be_wasm_dylink::ENV && name == be_wasm_dylink::MEMORY_NAME && kind == "memory";
        if !transport && !memory {
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

/// The pages of the memory an instance of `memory` gets, initial and maximum, capped at
/// `limit` bytes, or why the module needs more than that from the start.
fn capped(memory: DefinedMemory, limit: usize) -> Result<(u64, u64), String> {
    let cap = limit as u64 / u64::from(be_wasm_dylink::PAGE_SIZE);
    if memory.initial > cap {
        return Err(format!(
            "the guest's memory starts at {} pages, past its limit of {cap}",
            memory.initial
        ));
    }
    Ok((
        memory.initial,
        memory.maximum.map_or(cap, |maximum| maximum.min(cap)),
    ))
}

/// How an instance runs.
pub struct Config {
    /// Its name, which says which instance could not start.
    pub name: String,
    /// The most bytes its memory may hold. A module from bytes gets a memory that grows no
    /// further; the browser cannot hold a staged app's worker to it, whose memory is the most its
    /// own declares.
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
        Source::Module(module) => module
            .compiled
            .memory
            .map(|memory| capped(memory, config.memory_limit))
            .transpose()
            .and_then(|memory| {
                WebGuest::start_module(&module.compiled.module, memory, wake, Arc::clone(&waiting))
            }),
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
    /// Why it is not settled, if it is not: settled means it has nothing left to do with what it
    /// was sent and nothing it sent waits to be taken (`receive`): no frames wait for it to start
    /// or for a step, and no step on them is in flight. One that stopped has only what it sent
    /// before it stopped left.
    pub fn unsettled(&self) -> Option<&'static str> {
        if self.worker.has_received() {
            return Some("it sent frames no one took");
        }
        if self.failure.is_some() {
            return None;
        }
        if self.worker.starting_with_frames() {
            return Some("frames wait for it to start");
        }
        self.worker.unsettled()
    }

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
