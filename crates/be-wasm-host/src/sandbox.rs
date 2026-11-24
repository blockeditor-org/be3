//! The sandbox (plans/host_as_platform_adapter.md, The sandbox API): instances, each on a thread
//! and isolate of its own, talking to their spawner by frames over a `be_wasm_guest_host::Channel`.
//! An instance runs either
//!
//! - a module from bytes: a pure guest's module compiled once per process, named by the SHA-256
//!   of its bytes, on a thread of its own, given only the transport. What it may do beside
//!   computing, its spawner serves it over its frames; or
//! - a staged app's module (`Staged`): the app itself or one of its workers, linked against the
//!   libraries it names and given WASI, as the host runs that app. Given a display
//!   (`Config::display`), it draws on the host's device, and what it presents stays there, named
//!   to the spawner only by the instance and the surface (`Instance::surface`). Without one it has
//!   no gpu, which a worker does not need, since it sends what it made back as bytes
//!   (guides/apps/runtime.md, Backends).
//!
//! A module from bytes never reaches the code cache (cache.rs): it is compiled anew in each
//! process and freed once nothing holds it. A stopped instance's thread is reaped lazily by the
//! thread that dropped it (`be_wasm_guest_host::thread::wait_for_workers`), and nothing waits for
//! it at exit.

use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Condvar, LazyLock, Mutex, Weak, atomic::AtomicBool},
    time::Duration,
};

use be_wasm_guest_host::{Deadline, Wake, Worker, thread::Threaded};

use crate::{App, Code, Host, PresentedTexture, Presenting, Stored, engine};

/// A module, by the SHA-256 of the bytes it was compiled from.
pub type ModuleId = [u8; 32];

/// A compiled module, which any number of instances run without compiling it again.
#[derive(Clone)]
pub struct Module {
    id: ModuleId,
    pub(crate) compiled: engine::Module,
}

impl Module {
    pub fn id(&self) -> ModuleId {
        self.id
    }
}

/// Every module compiled and still held, so the same bytes compile once.
static COMPILED: LazyLock<Mutex<HashMap<ModuleId, Weak<engine::Compiled>>>> =
    LazyLock::new(Mutex::default);

/// A compile under way, which finishes on a thread of its own.
pub struct Compiling {
    slot: Arc<Slot>,
}

struct Slot {
    done: Mutex<Option<Result<Module, String>>>,
    finished: Condvar,
    wake: Wake,
}

impl Slot {
    fn finish(&self, outcome: Result<Module, String>) {
        *self.done.lock().unwrap_or_else(|held| held.into_inner()) = Some(outcome);
        self.finished.notify_all();
        (self.wake)();
    }
}

impl Compiling {
    /// The module, or why `bytes` are not a pure guest's module, once compiled.
    pub fn done(&self) -> Option<Result<Module, String>> {
        self.slot
            .done
            .lock()
            .unwrap_or_else(|held| held.into_inner())
            .clone()
    }

    /// Waits up to `timeout` for the compile to finish. Only a host thread that may block calls
    /// this; a guest's never does.
    pub fn wait(&self, timeout: Duration) -> Option<Result<Module, String>> {
        let done = self
            .slot
            .done
            .lock()
            .unwrap_or_else(|held| held.into_inner());
        let (done, _) = self
            .slot
            .finished
            .wait_timeout_while(done, timeout, |done| done.is_none())
            .unwrap_or_else(|held| held.into_inner());
        done.clone()
    }
}

/// Compiles a pure guest's module (guides/app_libraries.md, Pure guests) from `bytes` on a thread
/// of its own, or finds it compiled already, and calls `wake` once it is done.
pub fn compile(bytes: Vec<u8>, wake: Wake) -> Compiling {
    let id = be_wasm_dylink::sha256(&bytes);
    let slot = Arc::new(Slot {
        done: Mutex::new(None),
        finished: Condvar::new(),
        wake,
    });
    if let Some(module) = compiled(id) {
        slot.finish(Ok(module));
        return Compiling { slot };
    }
    let finishing = Arc::clone(&slot);
    let started = std::thread::Builder::new()
        .name("wasm compile".to_owned())
        .spawn(move || finishing.finish(compile_now(id, bytes)));
    if let Err(error) = started {
        slot.finish(Err(format!(
            "the module's compile could not start: {error}"
        )));
    }
    Compiling { slot }
}

fn compiled(id: ModuleId) -> Option<Module> {
    let compiled = COMPILED
        .lock()
        .unwrap_or_else(|held| held.into_inner())
        .get(&id)?
        .upgrade()?;
    Some(Module { id, compiled })
}

fn compile_now(id: ModuleId, bytes: Vec<u8>) -> Result<Module, String> {
    let bytes = crate::source(bytes)?;
    let bytes = engine::pure(&bytes, None)?;
    let (bytes, _) = be_wasm_dylink::importing_memory(&bytes)?;
    let engine::Isolate {
        shared: isolate,
        context,
        exhausted,
    } = engine::isolate();
    let made = {
        let mut locker = isolate.lock();
        v8::scope!(let scope, &mut *locker);
        let local = v8::Local::new(scope, &context);
        let scope = &mut v8::ContextScope::new(scope, local);
        engine::compile_uncached(scope, &bytes)
    };
    drop(context);
    drop(isolate);
    drop(exhausted);
    let compiled = made?;
    let mut held = COMPILED.lock().unwrap_or_else(|held| held.into_inner());
    held.retain(|_, module| module.strong_count() > 0);
    // Two compiles of the same bytes may have raced: the first one kept is the one shared.
    let compiled = match held.get(&id).and_then(Weak::upgrade) {
        Some(kept) => kept,
        None => {
            held.insert(id, Arc::downgrade(&compiled));
            compiled
        }
    };
    Ok(Module { id, compiled })
}

/// How an instance runs.
pub struct Config {
    /// Its thread's name, which logs and the host's reports show.
    pub name: String,
    /// The most bytes its memory may hold.
    pub memory_limit: usize,
    /// The device a staged app draws on: the host's, so what it presents never leaves the GPU.
    /// None gives it no display, so its gpu calls fail. A module from bytes never has one.
    pub display: Option<(wgpu::Device, wgpu::Queue)>,
}

/// What an instance runs.
pub enum Source {
    /// A module compiled from bytes, given only the transport.
    Module(Module),
    /// A staged app.
    App(Staged),
}

/// Reads a staged app's files by name: its module and the libraries it is linked against
/// (`library_file_name`).
pub type Open = Arc<dyn Fn(&str) -> Result<Stored, String> + Send + Sync>;

/// A staged app's module, `entry` (its entry point or one of its workers), and where it and its
/// libraries are read from, as the host's staged-app loading finds them (be-host-runtime's
/// `staged_app`).
#[derive(Clone)]
pub struct Staged {
    entry: String,
    open: Open,
}

impl Staged {
    pub fn new(entry: String, open: Open) -> Self {
        Self { entry, open }
    }

    /// The module's name among the staged files, which `open` reads it by.
    pub fn entry(&self) -> &str {
        &self.entry
    }

    /// Loads the app on `host`, linked against the libraries it names.
    pub fn load(&self, host: &Host) -> Result<App, String> {
        host.load_from(&self.entry, |name| (self.open)(name))
    }
}

const NO_DISPLAY: &str = "an app spawned in the sandbox has no display";

/// An instance of a source on a thread and isolate of its own. Dropping it stops it, even
/// mid-step, by terminating its isolate.
pub struct Instance {
    worker: Worker<Threaded<PresentedTexture>>,
    /// Why it stopped, once it did.
    failure: Option<String>,
    /// What each of its surfaces shows: the texture it presented last, until it lets the surface
    /// go.
    presented: BTreeMap<u32, PresentedTexture>,
}

/// Starts an instance of `source`, which calls `wake` whenever it has sent frames or stopped. A
/// staged app that cannot be read or linked starts, and stops at once saying why.
pub fn spawn(source: Source, config: Config, wake: Wake) -> Result<Instance, String> {
    let waiting = Arc::new(AtomicBool::new(false));
    let host = match &source {
        Source::Module(_) => Host::pure()?,
        Source::App(_) => match config.display {
            Some((device, queue)) => Host::new(device, queue)?,
            None => Host::on_demand(|| Err(NO_DISPLAY.to_owned()))?,
        },
    };
    let channel = host.with_memory_limit(config.memory_limit).spawn(
        config.name,
        move |host| match source {
            Source::Module(module) => {
                App::new(host, Code::Compiled(module.compiled), &no_library).map(Presenting)
            }
            Source::App(staged) => staged.load(host).map(Presenting),
        },
        wake,
        Arc::clone(&waiting),
        be_wasm_guest_host::thread::catch,
    )?;
    Ok(Instance {
        worker: Worker::new(channel, waiting),
        failure: None,
        presented: BTreeMap::new(),
    })
}

fn no_library(identity: &str) -> Result<Vec<u8>, String> {
    Err(format!(
        "a module from bytes links no library, and it names {identity}"
    ))
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

    /// Why the instance is still working on frames it was sent (`Worker::unsettled`): a step on
    /// them in flight, or frames waiting for one. None once it stopped.
    pub fn unsettled(&self) -> Option<&'static str> {
        match self.failure {
            Some(_) => None,
            None => self.worker.unsettled(),
        }
    }

    /// The frames the instance sent since the last call, or, once it sent nothing more, why it
    /// stopped.
    pub fn receive(&mut self) -> Result<Vec<Vec<u8>>, String> {
        if self.failure.is_none() {
            self.failure = self.worker.poll();
        }
        for surface in self.worker.take_released() {
            self.presented.remove(&surface);
        }
        if let Some((presented, _)) = self.worker.take_presented() {
            for presented in presented {
                self.presented.insert(presented.surface, presented);
            }
        }
        let received = self.worker.take_received();
        match &self.failure {
            Some(failure) if received.is_empty() => Err(failure.clone()),
            _ => Ok(received),
        }
    }

    /// What the instance's surface `surface` shows: the texture it presented last, on the device
    /// its display gave it, or None if it never presented that surface.
    pub fn surface(&self, surface: u32) -> Option<&PresentedTexture> {
        self.presented.get(&surface)
    }

    /// Waits until the instance sends a frame or `deadline` passes, and answers what it sent:
    /// nothing if the deadline passed first. Only a host thread that may block calls this.
    pub fn wait(&mut self, deadline: Deadline) -> Result<Vec<Vec<u8>>, String> {
        loop {
            let received = self.receive()?;
            if !received.is_empty() {
                return Ok(received);
            }
            match self.worker.wait(deadline) {
                Ok(true) => {}
                Ok(false) => return Ok(Vec::new()),
                Err(failure) => {
                    self.failure = Some(failure);
                }
            }
        }
    }
}
