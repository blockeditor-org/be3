//! The host every wasm guest runs on: apps (with the gpu and WASI), pure guests (type modules:
//! transport only, and a memory cap) and their libraries, on V8 (decisions/guests_run_on_v8.md).
//! Each guest is single-threaded (decisions/guests_are_single_threaded.md) and has an isolate of
//! its own; modules are compiled once and shared between isolates; the gpu and transport imports
//! are fast-API calls where memories stay put (gpu.rs).

mod cache;
mod dylink;
pub(crate) mod engine;
mod gauge;
mod gpu;
mod harness;
mod instance;
mod interrupt;
mod panic;
pub mod sandbox;
mod state;
mod wake;
mod wasi;

#[cfg(test)]
mod tests;

use std::{
    collections::{HashMap, HashSet},
    convert::Infallible,
    mem::ManuallyDrop,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::{Arc, Mutex, atomic::AtomicBool},
    time::{Duration, Instant},
};

use be_wasm_guest_host::{
    Produced, StepTime, Wake as HostWake,
    thread::{self, Contain, Runnable, Spawn, Threaded},
};

use be_wasm_gpu_host::Gpu;

use dylink::Program;
use engine::{Kind, Module};
use instance::{Answers, Role};
use interrupt::Interrupt;
use state::{Device, Memory};
use wake::Wake;
use wasi::Wasi;

pub use be_wasm_dylink::library_file_name;
pub use be_wasm_gpu_host::Imports;
pub use cache::{cache_code_in, persist};
pub use gauge::memories;
pub use interrupt::Interrupter;
pub use state::{Connect, State};

const INTERRUPTED: &str = "the app was stopped while it ran";

const CONSTRUCTORS: &str = "__wasm_call_ctors";

const INITIALIZE: &str = "_initialize";

const MEMORY: &str = "memory";

pub const MEMORY_LIMIT: usize = 1 << 30;

type Libraries = Arc<Mutex<HashMap<String, Module>>>;

#[derive(Clone)]
pub struct Host {
    libraries: Libraries,
    devices: Devices,
    imports: Imports,
    interrupt: Arc<Interrupt>,
    flavor: Flavor,
    memory_limit: Option<usize>,
    /// What the pure guests it loads draw their randomness from (`with_seed`).
    seed: [u8; 32],
    name: Option<String>,
}

#[derive(Clone)]
enum Devices {
    Given(wgpu::Device, wgpu::Queue),
    Awaited(Connect),
    OnDemand(Connect),
    None,
}

impl Devices {
    fn open(&self, imports: &Imports) -> Device {
        match self {
            Self::Given(device, queue) => Device::Ready(Box::new(
                Gpu::new(device.clone(), queue.clone()).sharing(Arc::clone(imports)),
            )),
            Self::Awaited(connect) => Device::Unconnected {
                connect: Arc::clone(connect),
                wakes: false,
            },
            Self::OnDemand(connect) => Device::Unconnected {
                connect: Arc::clone(connect),
                wakes: true,
            },
            Self::None => Device::Unavailable("a pure guest has no gpu".to_owned()),
        }
    }
}

impl Host {
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Result<Self, String> {
        Self::with(Devices::Given(device, queue), Flavor::App)
    }

    pub fn awaiting(
        device: impl Fn() -> Result<(wgpu::Device, wgpu::Queue), String> + Send + Sync + 'static,
    ) -> Result<Self, String> {
        Self::with(Devices::Awaited(Arc::new(device)), Flavor::App)
    }

    pub fn on_demand(
        connect: impl Fn() -> Result<(wgpu::Device, wgpu::Queue), String> + Send + Sync + 'static,
    ) -> Result<Self, String> {
        Self::with(Devices::OnDemand(Arc::new(connect)), Flavor::App)
    }

    pub fn pure() -> Result<Self, String> {
        Self::with(Devices::None, Flavor::Pure)
    }

    fn with(devices: Devices, flavor: Flavor) -> Result<Self, String> {
        engine::initialize();
        Ok(Self {
            interrupt: Interrupt::new(),
            libraries: libraries(flavor),
            devices,
            imports: Imports::default(),
            flavor,
            memory_limit: (flavor == Flavor::Pure).then_some(MEMORY_LIMIT),
            seed: [0; 32],
            name: None,
        })
    }

    /// Gives the pure guests this host loads the host's `device` to draw on, as an app draws:
    /// what they present stays on it, and the host reads it by surface (`App::surface`). Only
    /// the root guest is given one (plans/host_as_platform_adapter.md, The layer list).
    pub fn with_display(mut self, device: wgpu::Device, queue: wgpu::Queue) -> Self {
        self.devices = Devices::Given(device, queue);
        self
    }

    /// Seeds the randomness of the pure guests this host loads (be-wasm-pure-wasi's
    /// `random_get`), which is the zero seed's unless given.
    pub fn with_seed(mut self, seed: [u8; 32]) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_memory_limit(mut self, bytes: usize) -> Self {
        self.memory_limit = Some(bytes);
        self
    }

    pub fn imports(&self) -> Imports {
        Arc::clone(&self.imports)
    }

    pub fn interrupter(&self) -> Interrupter {
        Interrupter::new(&self.interrupt)
    }

    pub fn spawn<R: Runnable + 'static>(
        mut self,
        name: String,
        load: impl FnOnce(&Host) -> Result<R, String> + Send + 'static,
        wake: HostWake,
        waiting: Arc<AtomicBool>,
        contain: Contain,
    ) -> Result<Threaded<R::Presented>, String> {
        let interrupter = self.interrupter();
        self.name = Some(name.clone());
        thread::spawn(
            Spawn {
                name,
                wake,
                waiting,
                interrupt: Box::new(move || interrupter.interrupt()),
                contain,
            },
            move || load(&self),
        )
    }

    pub fn load(&self, path: &Path) -> Result<App, String> {
        let directory = path.parent().unwrap_or(Path::new("."));
        let entry = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{} names no module", path.display()))?;
        self.load_from(entry, |name| Stored::file(directory.join(name)))
    }

    pub fn load_from(
        &self,
        entry: &str,
        read: impl Fn(&str) -> Result<Stored, String>,
    ) -> Result<App, String> {
        App::new(self, Code::Bytes(read(entry)?.into_bytes()?), &|identity| {
            read(&library_file_name(identity))?.into_bytes()
        })
    }

    pub fn load_bytes(&self, bytes: &[u8]) -> Result<App, String> {
        self.load_bytes_with_libraries(bytes, |identity| {
            Err(format!(
                "the app needs the library {identity}, and none was given"
            ))
        })
    }

    pub fn load_bytes_with_libraries(
        &self,
        bytes: &[u8],
        library: impl Fn(&str) -> Result<Vec<u8>, String>,
    ) -> Result<App, String> {
        App::new(self, Code::Bytes(bytes.to_vec()), &library)
    }

    /// The program `app` is, with each library it is linked against compiled once for every
    /// isolate of the host's flavor.
    fn program(
        &self,
        scope: &mut v8::PinScope<'_, '_>,
        app: Module,
        library: &dyn Fn(&str) -> Result<Vec<u8>, String>,
    ) -> Result<(Program, Vec<String>), String> {
        let identities = dylink::libraries(&app.shape)?;
        if identities.is_empty() {
            return Ok((Program::Whole(app), identities));
        }
        let mut sources = Vec::with_capacity(identities.len());
        for identity in &identities {
            sources.push(self.source(identity, library)?);
        }
        // Each library keeps the exports a module above it imports (engine::trimmed): every staged
        // app's (keep_exports), so that the apps share one compile of it, and this one's.
        let mut kept: HashSet<String> = staged_imports();
        kept.extend(app.shape.imports.iter().map(|import| import.name.clone()));
        let mut trimmed = Vec::with_capacity(identities.len());
        for index in (0..identities.len()).rev() {
            // A library's own imports too: dylink resolves its GOT.func and GOT.mem entries
            // against its own exports first.
            kept.extend(
                sources[index]
                    .1
                    .imports
                    .iter()
                    .map(|import| import.name.clone()),
            );
            let mut names: Vec<&str> = kept.iter().map(String::as_str).collect();
            names.sort_unstable();
            let key = format!(
                "{}:{}",
                identities[index],
                be_wasm_dylink::identity(names.join("\n").as_bytes())
            );
            let cached = self
                .libraries
                .lock()
                .map_err(|_| "the library cache was poisoned".to_owned())?
                .contains_key(&key);
            let bytes = match cached {
                true => Ok(Vec::new()),
                false => engine::trimmed(&sources[index].0, |name| kept.contains(name)),
            };
            trimmed.push((key, bytes));
        }
        trimmed.reverse();
        let mut modules = Vec::with_capacity(identities.len() + 1);
        for (identity, (key, bytes)) in identities.iter().zip(trimmed) {
            let mut libraries = self
                .libraries
                .lock()
                .map_err(|_| "the library cache was poisoned".to_owned())?;
            let module = match libraries.get(&key) {
                Some(module) => Arc::clone(module),
                None => {
                    let module = bytes
                        .and_then(|bytes| engine::compile(scope, &bytes))
                        .map_err(|error| {
                            format!("the library {identity} could not be loaded: {error}")
                        })?;
                    libraries.insert(key, Arc::clone(&module));
                    module
                }
            };
            modules.push(module);
        }
        modules.push(app);
        Ok((Program::Linked(modules.into()), identities))
    }
}

impl Host {
    /// A library's bytes, checked against its identity once, and what it imports and exports.
    fn source(
        &self,
        identity: &str,
        library: &dyn Fn(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Arc<(Vec<u8>, engine::Shape)>, String> {
        static SOURCES: Mutex<Vec<(String, Arc<(Vec<u8>, engine::Shape)>)>> =
            Mutex::new(Vec::new());
        let mut sources = SOURCES.lock().unwrap_or_else(|held| held.into_inner());
        if let Some((_, found)) = sources.iter().find(|(known, _)| known == identity) {
            return Ok(Arc::clone(found));
        }
        let loaded = library(identity)
            .and_then(|bytes| {
                be_wasm_dylink::check_library(&bytes, identity)?;
                let bytes = source(bytes)?;
                let shape = engine::Shape::read(&bytes)?;
                Ok(Arc::new((bytes, shape)))
            })
            .map_err(|error| format!("the library {identity} could not be loaded: {error}"))?;
        sources.push((identity.to_owned(), Arc::clone(&loaded)));
        Ok(loaded)
    }
}

static STAGED: Mutex<Option<Arc<HashSet<String>>>> = Mutex::new(None);

/// Every name the staged apps import, which the libraries they link keep among their exports (and
/// no other function or global; engine::trimmed). Without it, a library keeps what each app that
/// loads it imports, and apps importing differently each compile it again.
pub fn keep_exports(names: impl IntoIterator<Item = String>) {
    *STAGED.lock().unwrap_or_else(|held| held.into_inner()) =
        Some(Arc::new(names.into_iter().collect()));
}

fn staged_imports() -> HashSet<String> {
    STAGED
        .lock()
        .unwrap_or_else(|held| held.into_inner())
        .as_ref()
        .map(|staged| staged.as_ref().clone())
        .unwrap_or_default()
}

/// The names a module imports, read from no more of it than holds its imports.
pub fn imported_names(stored: Stored) -> Result<Vec<String>, String> {
    let bytes = match stored {
        Stored::Bytes(bytes) => bytes,
        Stored::File(path) => {
            use std::io::Read;
            let mut file = std::fs::File::open(&path)
                .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
            let mut bytes = Vec::new();
            loop {
                let read = (&mut file)
                    .take(1 << 20)
                    .read_to_end(&mut bytes)
                    .map_err(|error| format!("{} could not be read: {error}", path.display()))?;
                if let Some(names) = engine::imports_in(&bytes) {
                    return Ok(names);
                }
                if read == 0 {
                    break bytes;
                }
            }
        }
    };
    engine::imports_in(&bytes).ok_or_else(|| "the module's imports could not be read".to_owned())
}

/// A module's bytes as V8 takes them. The tests write their guests as text.
fn source(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    #[cfg(test)]
    return wat::parse_bytes(&bytes)
        .map(|bytes| bytes.into_owned())
        .map_err(|error| format!("the module is not valid: {error}"));
    #[cfg(not(test))]
    Ok(bytes)
}

/// Refuses a guest that would have a second thread: one whose modules share a memory or ask for
/// threads (`wasi.thread-spawn`), since a guest is single-threaded
/// (decisions/guests_are_single_threaded.md).
fn single_threaded(program: &Program) -> Result<(), String> {
    const SINGLE: &str = "a guest is single-threaded (decisions/guests_are_single_threaded.md)";
    for module in program.modules() {
        if module.shape.shared {
            return Err(format!("the app has a shared memory, and {SINGLE}"));
        }
        if module
            .shape
            .imports
            .iter()
            .any(|import| import.module == "wasi" && import.name == "thread-spawn")
        {
            return Err(format!("the app asks to start threads, and {SINGLE}"));
        }
    }
    Ok(())
}

/// A guest instantiated on an isolate of its own, which an App runs and a test run runs
/// (harness.rs).
pub(crate) struct Running {
    isolate: ManuallyDrop<v8::SharedIsolate>,
    /// Kept until after the isolate, whose heap watch reaches it.
    exhausted: Arc<engine::Exhausted>,
    context: ManuallyDrop<v8::Global<v8::Context>>,
    exports: ManuallyDrop<v8::Global<v8::Object>>,
    /// Owned here, freed after the isolate; the isolate's host functions reach it
    /// (state::state_of), so it is never borrowed across a call into the guest.
    state: NonNull<State>,
}

// the State and the isolate's handles move with the guest, and only the thread that
// holds it reaches them.
// unsafe(send-sync)
unsafe impl Send for Running {}

/// What a guest is given beside what its host gives every guest.
pub(crate) struct Setup {
    pub(crate) wasi: Arc<Wasi>,
    /// Whether a whole module's `_initialize` or constructors run once it is instantiated (a
    /// command's `_start` runs them itself).
    pub(crate) initialize: bool,
}

/// What a guest runs: a module's bytes, or a module the sandbox compiled (sandbox.rs).
pub(crate) enum Code {
    Bytes(Vec<u8>),
    Compiled(Module),
}

impl Running {
    pub(crate) fn new(
        host: &Host,
        code: Code,
        library: &dyn Fn(&str) -> Result<Vec<u8>, String>,
        wake: &Arc<Wake>,
        setup: Setup,
    ) -> Result<Self, String> {
        if host.interrupt.interrupted() {
            return Err(INTERRUPTED.to_owned());
        }
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
            v8::tc_scope!(let caught, scope);
            let made = Self::make(caught, host, &isolate, code, library, wake, setup);
            made.map_err(|error| match caught.has_caught() {
                true => format!("{error}: {}", crate::engine::thrown!(caught)),
                false => error,
            })
        };
        match made {
            Ok((state, exports)) => Ok(Self {
                isolate: ManuallyDrop::new(isolate),
                exhausted,
                context: ManuallyDrop::new(context),
                exports: ManuallyDrop::new(exports),
                state,
            }),
            Err(error) => {
                drop(context);
                drop(isolate);
                Err(error)
            }
        }
    }

    fn make(
        scope: &mut v8::PinScope<'_, '_>,
        host: &Host,
        isolate: &v8::SharedIsolate,
        code: Code,
        library: &dyn Fn(&str) -> Result<Vec<u8>, String>,
        wake: &Arc<Wake>,
        setup: Setup,
    ) -> Result<(NonNull<State>, v8::Global<v8::Object>), String> {
        let app = match code {
            Code::Bytes(bytes) => {
                let bytes = source(bytes)?;
                let bytes = match host.flavor == Flavor::Pure {
                    true => engine::pure(&bytes, host.memory_limit)?,
                    false => bytes,
                };
                engine::compile(scope, &bytes)?
            }
            Code::Compiled(module) => module,
        };
        let (program, identities) = host.program(scope, app, library)?;
        single_threaded(&program)?;
        let cap = host.memory_limit.map(engine::pages);
        let declared = match program.first().shape.memory_import() {
            Some(engine::Import {
                kind: Kind::Memory(declared),
                ..
            }) => Some(declared.clone()),
            _ => None,
        };
        let memory = match &declared {
            Some(declared) => {
                let maximum = match cap {
                    Some(cap) => Some(engine::capped_maximum(declared, cap)?),
                    None => declared.maximum,
                };
                Some(engine::memory(scope, declared.initial, maximum)?)
            }
            None => None,
        };
        if matches!(program, Program::Linked(_)) && memory.is_none() {
            return Err("the root library does not import its memory".to_owned());
        }
        let handle = Arc::new(isolate.thread_safe_handle());
        host.interrupt.watch(&handle);
        let state = State {
            memory: match memory {
                Some(memory) => Memory::of(scope, memory),
                None => Memory::None,
            },
            device: host.devices.open(&host.imports),
            imports: Arc::clone(&host.imports),
            error: None,
            fatal: None,
            panicked: None,
            returned: Vec::new(),
            transport: Default::default(),
            interrupt: Arc::clone(&host.interrupt),
            wake: Arc::clone(wake),
            wasi: setup.wasi,
            gpu_time: Duration::ZERO,
            submit_time: Duration::ZERO,
            isolate: handle,
        };
        let state = NonNull::from(Box::leak(Box::new(state)));
        let made = Self::instantiate(
            scope,
            host,
            &program,
            &identities,
            memory,
            state,
            setup.initialize,
        );
        match made {
            Ok(exports) => Ok((state, v8::Global::new(scope, exports))),
            Err(error) => {
                // nothing the failed instance holds runs again.
                // unsafe(memory)
                drop(unsafe { Box::from_raw(state.as_ptr()) });
                Err(error)
            }
        }
    }

    fn instantiate<'s>(
        scope: &mut v8::PinScope<'s, '_>,
        host: &Host,
        program: &Program,
        identities: &[String],
        memory: Option<v8::Local<'s, v8::Object>>,
        state: NonNull<State>,
        initialize_whole: bool,
    ) -> Result<v8::Local<'s, v8::Object>, String> {
        let pure = host.flavor == Flavor::Pure;
        let role = match pure {
            true => Role::Pure {
                gpu: !matches!(host.devices, Devices::None),
            },
            false => Role::App,
        };
        // A memory the module exports is known only once it is instantiated, and stays put as
        // one the host makes does.
        let fast = engine::memories_stay_put();
        let data = v8::External::new(scope, state.as_ptr().cast()).into();
        let answers = Answers {
            defined: instance::host_functions(scope, data, role, fast, program.modules())?,
            memory,
            pure,
        };
        let first = instance::instantiate_answered(scope, program.first(), &answers)?;
        match program {
            Program::Whole(_) => {
                // as above; the instance's start function, which may run the guest,
                // has returned.
                // unsafe(memory)
                let state = unsafe { &mut *state.as_ptr() };
                if let Memory::None = state.memory
                    && let Some(own) =
                        engine::get(scope, first, MEMORY).and_then(|memory| memory.to_object(scope))
                {
                    state.memory = Memory::of(scope, own);
                }
                if initialize_whole {
                    initialize(scope, first)?;
                }
                Ok(first)
            }
            Program::Linked(modules) => {
                let memory = memory.ok_or("the root library does not import its memory")?;
                initialize(scope, first)?;
                let linked = dylink::plan(scope, modules, identities, first, memory)?;
                dylink::link(scope, &answers, modules, &linked, first)
            }
        }
    }

    pub(crate) fn state(&self) -> &State {
        // the guest runs only inside Running's own calls, so nothing else reaches
        // the State while it is borrowed.
        // unsafe(memory)
        unsafe { self.state.as_ref() }
    }

    pub(crate) fn state_mut(&mut self) -> &mut State {
        // as in state.
        // unsafe(memory)
        unsafe { self.state.as_mut() }
    }

    /// Whether the guest exports a function `name`.
    pub(crate) fn exports(&self, name: &str) -> bool {
        self.with(|scope, exports| instance::function(scope, exports, name).is_some())
    }

    fn with<R>(
        &self,
        run: impl FnOnce(&mut v8::PinScope<'_, '_>, v8::Local<v8::Object>) -> R,
    ) -> R {
        let mut locker = self.isolate.lock();
        v8::scope!(let scope, &mut *locker);
        let context = v8::Local::new(scope, &*self.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        let exports = v8::Local::new(scope, &*self.exports);
        run(scope, exports)
    }

    /// Runs the guest's export `name` to its end: what it returned as an i32 when it returns
    /// one, or, when it did not return, what was thrown (empty when it was terminated). Either
    /// way the memory it holds is measured after.
    pub(crate) fn call(&mut self, name: &str) -> (Result<Option<i32>, String>, usize) {
        let state = self.state.as_ptr();
        let mut locker = self.isolate.lock();
        v8::scope!(let scope, &mut *locker);
        let context = v8::Local::new(scope, &*self.context);
        let scope = &mut v8::ContextScope::new(scope, context);
        v8::tc_scope!(let caught, scope);
        let exports = v8::Local::new(caught, &*self.exports);
        let Some(function) = instance::function(caught, exports, name) else {
            return (Err(format!("the app has no usable {name} export")), 0);
        };
        let receiver = v8::undefined(caught).into();
        let ran = function.call(caught, receiver, &[]);
        // the guest has returned.
        // unsafe(memory)
        let state = unsafe { &mut *state };
        let terminated = caught.has_terminated();
        if self.exhausted.reached() {
            state
                .fatal
                .get_or_insert_with(|| engine::OUT_OF_MEMORY.to_owned());
        }
        let outcome = match ran {
            Some(value) if state.fatal.is_none() => Ok(value.int32_value(caught)),
            _ => Err(match caught.has_caught() && !terminated {
                true => crate::engine::thrown!(caught),
                false => String::new(),
            }),
        };
        let size = state.view_in(caught).size();
        if terminated || state.fatal.is_some() {
            state.isolate.cancel_terminate_execution();
        }
        (outcome, size)
    }

    pub(crate) fn measure(&self) -> usize {
        self.with(|scope, _| self.state().view_in(scope).size())
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // each is dropped once, here, the handles before their isolate and the
        // State after it.
        // unsafe(memory)
        unsafe {
            ManuallyDrop::drop(&mut self.exports);
            ManuallyDrop::drop(&mut self.context);
            ManuallyDrop::drop(&mut self.isolate);
            drop(Box::from_raw(self.state.as_ptr()));
        }
    }
}

pub struct App {
    running: Running,
    wake: Arc<Wake>,
    stopped: bool,
    gauge: Arc<gauge::Gauge>,
}

impl App {
    fn new(
        host: &Host,
        code: Code,
        library: &dyn Fn(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let wake: Arc<Wake> = Arc::default();
        let setup = Setup {
            wasi: match host.flavor {
                Flavor::Pure => Wasi::pure(
                    host.name.clone().unwrap_or_else(|| "a pure guest".to_owned()),
                    host.seed,
                ),
                Flavor::App => Wasi::app(),
            },
            initialize: true,
        };
        let running = Running::new(host, code, library, &wake, setup)?;
        for name in [Call::Start, Call::Step, Call::Shutdown].map(Call::export) {
            if !running.exports(name) {
                return Err(format!("the app has no usable {name} export"));
            }
        }
        let app = Self {
            running,
            wake,
            stopped: false,
            gauge: gauge::Gauge::register(host.name.clone().unwrap_or_else(|| "wasm".to_owned())),
        };
        app.gauge.set(app.running.measure() as u64);
        Ok(app)
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.call(Call::Start)
    }

    pub fn step(&mut self) -> Result<(), String> {
        self.call(Call::Step)
    }

    pub fn on_wake(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.wake.notify(notify);
    }

    pub fn take_wake(&self) -> bool {
        self.wake.take()
    }

    pub(crate) fn state(&self) -> &State {
        self.running.state()
    }

    fn state_mut(&mut self) -> &mut State {
        self.running.state_mut()
    }

    /// Runs one of the guest's exports to its end, and says how it ended.
    fn run(&mut self, call: Call) -> Result<(), String> {
        let (outcome, size) = self.running.call(call.export());
        self.gauge.set(size as u64);
        outcome.map(|_| ())
    }

    fn call(&mut self, call: Call) -> Result<(), String> {
        if self.stopped {
            return Ok(());
        }
        if self.interrupted() {
            self.stopped = true;
            return Err(INTERRUPTED.to_owned());
        }
        if let Err(thrown) = self.run(call) {
            self.stopped = true;
            if self.interrupted() {
                return Err(INTERRUPTED.to_owned());
            }
            let state = self.state_mut();
            if let Some(fatal) = state.fatal.take() {
                return Err(fatal);
            }
            if let Some(message) = state.panicked.take() {
                eprintln!("be-wasm-host: an app panicked: {message}\n{thrown}");
                return Err(message);
            }
            return Err(format!("the app trapped: {thrown}"));
        }
        let state = self.state_mut();
        match state.take_error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn deliver(&mut self, frames: Vec<Vec<u8>>) {
        self.state_mut().transport.inbox.extend(frames);
    }

    pub fn take_outbound(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.state_mut().transport.outbox)
    }

    pub fn take_gpu_time(&mut self) -> (Duration, Duration) {
        let state = self.state_mut();
        (
            std::mem::take(&mut state.gpu_time),
            std::mem::take(&mut state.submit_time),
        )
    }

    pub fn take_presented(&mut self) -> Vec<u32> {
        self.state_mut()
            .device
            .ready_mut()
            .map(Gpu::take_presented)
            .unwrap_or_default()
    }

    /// The texture `surface` showed when the app last presented it, and its generation, kept
    /// while the app configures it afresh until it presents again.
    pub fn presented_surface(&self, surface: u32) -> Option<(wgpu::Texture, u64)> {
        self.state()
            .device
            .ready()?
            .presented_surface(surface)
            .map(|(texture, generation)| (texture.clone(), generation))
    }

    pub fn surface(&self, surface: u32) -> Option<(wgpu::Texture, u64)> {
        self.state()
            .device
            .ready()?
            .surface(surface)
            .map(|(texture, generation)| (texture.clone(), generation))
    }

    pub fn attach_surface(&mut self, surface: u32, texture: wgpu::Texture) {
        let state = self.state_mut();
        match state.gpu() {
            Ok(gpu) => gpu.attach_surface(surface, texture),
            Err(message) => {
                state.error.get_or_insert(message);
            }
        }
    }

    /// The surfaces the app or its host let go of since the last call.
    pub fn take_released(&mut self) -> Vec<u32> {
        self.state_mut()
            .device
            .ready_mut()
            .map(Gpu::take_released)
            .unwrap_or_default()
    }

    pub fn detach_surface(&mut self, surface: u32) {
        if let Some(gpu) = self.state_mut().device.ready_mut() {
            gpu.detach_surface(surface);
        }
    }

    pub fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        if !self.interrupted() {
            let _ = self.run(Call::Shutdown);
        }
    }

    fn interrupted(&self) -> bool {
        self.state().interrupt.interrupted()
    }
}

#[derive(Clone, Copy)]
enum Call {
    Start,
    Step,
    Shutdown,
}

impl Call {
    fn export(self) -> &'static str {
        match self {
            Self::Start => "app_start",
            Self::Step => "app_step",
            Self::Shutdown => "app_shutdown",
        }
    }
}

/// A surface an app presented: the texture it shows now, on the host's device, which the host
/// draws from without its pixels leaving the GPU, and its generation, which changes whenever the
/// app shows another texture.
#[derive(Clone)]
pub struct PresentedTexture {
    pub surface: u32,
    pub texture: wgpu::Texture,
    pub generation: u64,
}

impl be_wasm_guest_host::Presented for PresentedTexture {
    fn surface(&self) -> u32 {
        self.surface
    }
}

impl Runnable for App {
    type Presented = Infallible;

    fn start(&mut self) -> Result<Produced<Infallible>, String> {
        App::start(self)?;
        Ok(Produced::sent(self.take_outbound(), StepTime::default()))
    }

    fn step(&mut self, frames: Vec<Vec<u8>>) -> Result<Produced<Infallible>, String> {
        self.take_wake();
        let started = Instant::now();
        self.deliver(frames);
        App::step(self)?;
        let took = StepTime {
            step: started.elapsed(),
            ..StepTime::default()
        };
        Ok(Produced::sent(self.take_outbound(), took))
    }

    fn take_wake(&mut self) -> bool {
        App::take_wake(self)
    }

    fn on_wake(&self, wake: HostWake) {
        App::on_wake(self, move || wake());
    }

    fn stop(&mut self) {
        App::stop(self);
    }
}

/// An app with a display, run on a thread of its own: each step reports what it presented, with
/// the time its gpu calls and submits took. be-host-runtime runs its apps so, and the sandbox the
/// staged apps a root spawns.
pub struct Presenting(pub App);

impl Runnable for Presenting {
    type Presented = PresentedTexture;

    fn start(&mut self) -> Result<Produced<PresentedTexture>, String> {
        let sent = Runnable::start(&mut self.0)?;
        Ok(self.produced(sent))
    }

    fn step(&mut self, frames: Vec<Vec<u8>>) -> Result<Produced<PresentedTexture>, String> {
        let sent = Runnable::step(&mut self.0, frames)?;
        Ok(self.produced(sent))
    }

    fn take_wake(&mut self) -> bool {
        self.0.take_wake()
    }

    fn on_wake(&self, wake: HostWake) {
        Runnable::on_wake(&self.0, wake);
    }

    fn stop(&mut self) {
        self.0.stop();
    }
}

impl Presenting {
    /// What a step that `sent` produced, with the surfaces it presented, each once.
    fn produced(&mut self, sent: Produced<Infallible>) -> Produced<PresentedTexture> {
        let app = &mut self.0;
        let (gpu, submit) = app.take_gpu_time();
        let released = app.take_released();
        let mut presented: Vec<PresentedTexture> = Vec::new();
        for surface in app.take_presented() {
            let Some((texture, generation)) = app.presented_surface(surface) else {
                continue;
            };
            presented.retain(|held| held.surface != surface);
            presented.push(PresentedTexture {
                surface,
                texture,
                generation,
            });
        }
        Produced {
            painted: !presented.is_empty(),
            outbound: sent.outbound,
            presented,
            released,
            took: StepTime {
                gpu,
                submit,
                ..sent.took
            },
        }
    }
}

/// Runs the module's `_initialize`, or else its constructors, when it has them.
fn initialize(
    scope: &mut v8::PinScope<'_, '_>,
    exports: v8::Local<v8::Object>,
) -> Result<(), String> {
    let (name, failure) = match instance::function(scope, exports, INITIALIZE) {
        Some(_) => (INITIALIZE, "the app could not initialize itself"),
        None => (CONSTRUCTORS, "the app could not run its constructors"),
    };
    match instance::call_export(scope, exports, name, &[]) {
        None | Some(Ok(_)) => Ok(()),
        Some(Err(())) => Err(failure.to_owned()),
    }
}

pub enum Stored {
    File(PathBuf),
    Bytes(Vec<u8>),
}

impl Stored {
    pub fn file(path: PathBuf) -> Result<Self, String> {
        match path.is_file() {
            true => Ok(Self::File(path)),
            false => Err(format!("{} is not a file", path.display())),
        }
    }

    pub fn into_bytes(self) -> Result<Vec<u8>, String> {
        match self {
            Self::File(path) => std::fs::read(&path)
                .map_err(|error| format!("{} could not be read: {error}", path.display())),
            Self::Bytes(bytes) => Ok(bytes),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flavor {
    App,
    Pure,
}

/// The libraries compiled for every host of a flavor, so each is compiled once per process.
fn libraries(flavor: Flavor) -> Libraries {
    static LIBRARIES: Mutex<Vec<(Flavor, Libraries)>> = Mutex::new(Vec::new());
    let mut libraries = LIBRARIES.lock().unwrap_or_else(|held| held.into_inner());
    if let Some((_, found)) = libraries.iter().find(|(kind, _)| *kind == flavor) {
        return Arc::clone(found);
    }
    let made = Libraries::default();
    libraries.push((flavor, Arc::clone(&made)));
    made
}
