mod direct;
mod dylink;
mod gpu;
mod harness;
mod interrupt;
mod panic;
mod precompile;
mod pure;
mod state;
mod stderr;
mod threads;
mod wake;

#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use block_gpu_host::Gpu;
use wasmtime::{
    Cache, CacheConfig, Config, Engine, Instance, Linker, MemoryType, Module, SharedMemory, Store,
    TypedFunc,
};
use wasmtime_wasi::{WasiCtxBuilder, p1};

use dylink::Program;
use interrupt::Interrupt;
use state::{Device, Memory};
use threads::Spawner;
use wake::Wake;

pub use block_gpu_host::Imports;
pub use interrupt::Interrupter;
pub use precompile::{precompile, precompile_to};
pub use pure::{MEMORY_LIMIT, PureGuest, PureHost};
pub use state::{Connect, State};
pub use wasm_dylink::sdk_file_name;

pub const PRECOMPILED_EXTENSION: &str = "cwasm";

const INTERRUPTED: &str = "the plugin was stopped while it ran";

const CONSTRUCTORS: &str = "__wasm_call_ctors";

const INITIALIZE: &str = "_initialize";

const MEMORY: &str = "memory";

const BACKTRACE_FRAMES: usize = 100;

type Sdks = Arc<Mutex<HashMap<String, Module>>>;

#[derive(Clone)]
pub struct Host {
    engine: Engine,
    sdks: Sdks,
    devices: Devices,
    imports: Imports,
    direct: Vec<(String, String)>,
    interrupt: Arc<Interrupt>,
}

#[derive(Clone)]
enum Devices {
    Given(wgpu::Device, wgpu::Queue),
    OnDemand(Connect),
}

impl Devices {
    fn open(&self, imports: &Imports) -> Device {
        match self {
            Self::Given(device, queue) => Device::Ready(Box::new(
                Gpu::new(device.clone(), queue.clone()).sharing(Arc::clone(imports)),
            )),
            Self::OnDemand(connect) => Device::Unconnected(Arc::clone(connect)),
        }
    }
}

impl Host {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        cache: Option<&Path>,
    ) -> Result<Self, String> {
        let (engine, sdks) = engine(cache, Flavor::Plugin)?;
        Ok(Self {
            interrupt: Interrupt::new(engine.clone()),
            engine,
            sdks,
            devices: Devices::Given(device, queue),
            imports: Imports::default(),
            direct: Vec::new(),
        })
    }

    pub fn on_demand(
        connect: impl Fn() -> Result<(wgpu::Device, wgpu::Queue), String> + Send + Sync + 'static,
        cache: Option<&Path>,
    ) -> Result<Self, String> {
        let (engine, sdks) = engine(cache, Flavor::Plugin)?;
        Ok(Self {
            interrupt: Interrupt::new(engine.clone()),
            engine,
            sdks,
            devices: Devices::OnDemand(Arc::new(connect)),
            imports: Imports::default(),
            direct: Vec::new(),
        })
    }

    pub fn with_direct(mut self, imports: &[(&str, &str)]) -> Self {
        self.direct = imports
            .iter()
            .map(|(module, name)| ((*module).to_owned(), (*name).to_owned()))
            .collect();
        self
    }

    pub fn imports(&self) -> Imports {
        Arc::clone(&self.imports)
    }

    pub fn interrupter(&self) -> Interrupter {
        Interrupter::new(&self.interrupt)
    }

    pub fn load(&self, path: &Path) -> Result<Plugin, String> {
        let directory = path.parent().unwrap_or(Path::new("."));
        let entry = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{} names no module", path.display()))?;
        self.load_from(entry, |name| {
            let path = directory.join(name);
            std::fs::read(&path)
                .map_err(|error| format!("{} could not be read: {error}", path.display()))
        })
    }

    pub fn load_from(
        &self,
        entry: &str,
        read: impl Fn(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Plugin, String> {
        let plugin = match self.precompiled(entry, &read) {
            Some(plugin) => plugin,
            None => module(&self.engine, &read(entry)?)?,
        };
        self.link(plugin, |identity| {
            let sdk = sdk_file_name(identity);
            match self.precompiled(&sdk, &read) {
                Some(module) => Ok(module),
                None => sdk_module(&self.engine, identity, &read(&sdk)?),
            }
        })
    }

    fn precompiled(
        &self,
        name: &str,
        read: &impl Fn(&str) -> Result<Vec<u8>, String>,
    ) -> Option<Module> {
        let artifact = Path::new(name)
            .with_extension(PRECOMPILED_EXTENSION)
            .to_str()?
            .to_owned();
        let bytes = read(&artifact).ok()?;
        // unsafe(ffi)
        match unsafe { Module::deserialize(&self.engine, &bytes) } {
            Ok(module) => Some(module),
            Err(error) => {
                eprintln!("{artifact} could not be used, compiling the plugin instead: {error}");
                None
            }
        }
    }

    pub fn load_bytes(&self, bytes: &[u8]) -> Result<Plugin, String> {
        self.load_bytes_with_sdk(bytes, |identity| {
            Err(format!(
                "the plugin needs the plugin SDK {identity}, and none was given"
            ))
        })
    }

    pub fn load_bytes_with_sdk(
        &self,
        bytes: &[u8],
        sdk: impl FnOnce(&str) -> Result<Vec<u8>, String>,
    ) -> Result<Plugin, String> {
        let plugin = module(&self.engine, bytes)?;
        self.link(plugin, |identity| {
            sdk_module(&self.engine, identity, &sdk(identity)?)
        })
    }

    fn link(
        &self,
        plugin: Module,
        sdk: impl FnOnce(&str) -> Result<Module, String>,
    ) -> Result<Plugin, String> {
        let Some(identity) = dylink::sdk_identity(&plugin) else {
            return Plugin::new(self, Program::Whole(plugin));
        };
        let identity = identity?;
        let sdk = {
            let mut sdks = self
                .sdks
                .lock()
                .map_err(|_| "the plugin SDK cache was poisoned".to_owned())?;
            match sdks.get(&identity) {
                Some(module) => module.clone(),
                None => {
                    let module = sdk(&identity).map_err(|error| {
                        format!("the plugin SDK {identity} could not be loaded: {error}")
                    })?;
                    sdks.insert(identity, module.clone());
                    module
                }
            }
        };
        Plugin::new(self, Program::Linked { sdk, plugin })
    }
}

pub struct Plugin {
    store: Store<State>,
    instance: Instance,
    start: TypedFunc<(), ()>,
    step: TypedFunc<(), ()>,
    shutdown: TypedFunc<(), ()>,
    wake: Arc<Wake>,
    stopped: bool,
}

impl Plugin {
    fn new(host: &Host, program: Program) -> Result<Self, String> {
        if host.interrupt.interrupted() {
            return Err(INTERRUPTED.to_owned());
        }
        let engine = host.engine.clone();
        let imported = memory_import(program.first());
        let shared = match (&program, &imported) {
            (_, Some((_, _, declared))) if declared.is_shared() => Some(
                SharedMemory::new(&engine, declared.clone())
                    .map_err(|error| format!("the plugin memory could not be created: {error}"))?,
            ),
            (Program::Linked { .. }, _) => {
                return Err("the plugin SDK does not import a shared memory".to_owned());
            }
            _ => None,
        };
        let wasi = WasiCtxBuilder::new().inherit_stderr().build_p1();
        let wake: Arc<Wake> = Arc::default();
        let threads = Spawner::new(
            engine.clone(),
            program.clone(),
            shared.clone(),
            Arc::clone(&wake),
            Arc::clone(&host.interrupt),
        );
        let state = State {
            wasi,
            memory: shared.clone().map_or(Memory::Own(None), Memory::Shared),
            device: host.devices.open(&host.imports),
            imports: Arc::clone(&host.imports),
            error: None,
            fatal: None,
            returned: Vec::new(),
            transport: Default::default(),
            gpu_time: Duration::ZERO,
            submit_time: Duration::ZERO,
            threads: Arc::clone(&threads),
            wake: Arc::clone(&wake),
        };
        let mut store = Store::new(&engine, state);
        interrupt::watch(&mut store, true);
        let mut linker: Linker<State> = Linker::new(&engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut State| &mut state.wasi)
            .map_err(|error| format!("wasi could not be linked: {error}"))?;
        if let Some((module, name, declared)) = imported {
            let defined = match &shared {
                Some(shared) => linker.define(&store, &module, &name, shared.clone()),
                None => {
                    let own = wasmtime::Memory::new(&mut store, declared).map_err(|error| {
                        format!("the plugin memory could not be created: {error}")
                    })?;
                    store.data_mut().memory = Memory::Own(Some(own));
                    linker.define(&store, &module, &name, own)
                }
            };
            defined.map_err(|error| format!("the plugin memory could not be linked: {error}"))?;
        }
        gpu::link(&mut linker)?;
        panic::link(&mut linker)?;
        threads::link(&mut linker)?;
        wake::link(&mut linker)?;
        direct::link(&mut linker, &host.direct)?;
        linker
            .define_unknown_imports_as_traps(program.first())
            .map_err(|error| format!("the plugin imports could not be stubbed: {error}"))?;
        let first = linker
            .instantiate(&mut store, program.first())
            .map_err(|error| format!("the plugin could not be instantiated: {error}"))?;
        let instance = match (&program, &shared) {
            (Program::Whole(_), _) => {
                if let Memory::Own(None) = store.data().memory {
                    store.data_mut().memory = Memory::Own(first.get_memory(&mut store, MEMORY));
                }
                initialize(&first, &mut store)?;
                first
            }
            (Program::Linked { plugin, .. }, Some(memory)) => {
                call_constructors(&first, &mut store)?;
                let mut layout = dylink::plan(&mut store, &first, plugin, memory)?;
                let instance = dylink::instantiate(&mut store, &linker, &first, plugin, &layout)?;
                dylink::relocate(&mut store, &instance, &mut layout)?;
                initialize_storage(&first, &instance, &mut store)?;
                let base = dylink::main_thread_locals(&mut store, &first)?;
                dylink::place_thread_locals(&mut store, &instance, memory, &layout, base)?;
                threads.linked(layout);
                call_constructors(&instance, &mut store)?;
                instance
            }
            (Program::Linked { .. }, None) => {
                return Err("the plugin SDK does not import a shared memory".to_owned());
            }
        };
        let plugin = Self {
            start: typed(&instance, &mut store, "plugin_start")?,
            step: typed(&instance, &mut store, "plugin_step")?,
            shutdown: typed(&instance, &mut store, "plugin_shutdown")?,
            store,
            instance,
            wake,
            stopped: false,
        };
        Ok(plugin)
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.call(self.start.clone())
    }

    pub fn step(&mut self) -> Result<(), String> {
        self.call(self.step.clone())
    }

    pub fn on_wake(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.wake.notify(notify);
    }

    pub fn take_wake(&self) -> bool {
        self.wake.take()
    }

    fn call(&mut self, function: TypedFunc<(), ()>) -> Result<(), String> {
        if self.stopped {
            return Ok(());
        }
        if self.interrupted() {
            self.stopped = true;
            return Err(INTERRUPTED.to_owned());
        }
        if let Some(failure) = self.store.data_mut().threads.take_failure() {
            self.stopped = true;
            return Err(failure);
        }
        let outcome = function.call(&mut self.store, ()).map_err(|error| {
            if self.store.data().threads.halts(true) {
                return INTERRUPTED.to_owned();
            }
            panic::message(&error).unwrap_or_else(|| format!("the plugin trapped: {error}"))
        });
        if let Err(error) = outcome {
            self.stopped = true;
            return Err(error);
        }
        let state = self.store.data_mut();
        match state.take_error().or_else(|| state.threads.take_failure()) {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub fn send(&mut self, frame: Vec<u8>) {
        self.store.data_mut().transport.inbox.push_back(frame);
    }

    pub fn deliver(&mut self, items: Vec<Vec<u8>>) -> Result<(), String> {
        for item in items {
            let Some(block_guest_abi::direct::Item::Call(call)) =
                block_guest_abi::direct::decode(&item)
            else {
                self.send(item);
                continue;
            };
            if !self.store.data().transport.inbox.is_empty() {
                self.step()?;
            }
            if self.stopped {
                return Ok(());
            }
            let returned =
                direct::call(&self.instance, &mut self.store, &call).inspect_err(|_| {
                    self.stopped = true;
                })?;
            if call.answered {
                self.store
                    .data_mut()
                    .transport
                    .outbox
                    .push(block_guest_abi::direct::returned(&returned));
            }
        }
        Ok(())
    }

    pub fn take_outbound(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.store.data_mut().transport.outbox)
    }

    pub fn take_gpu_time(&mut self) -> (Duration, Duration) {
        let state = self.store.data_mut();
        (
            std::mem::take(&mut state.gpu_time),
            std::mem::take(&mut state.submit_time),
        )
    }

    pub fn take_presented(&mut self) -> Vec<u32> {
        self.store
            .data_mut()
            .device
            .ready_mut()
            .map(Gpu::take_presented)
            .unwrap_or_default()
    }

    pub fn surface(&self, surface: u32) -> Option<(wgpu::Texture, u64)> {
        self.store
            .data()
            .device
            .ready()?
            .surface(surface)
            .map(|(texture, generation)| (texture.clone(), generation))
    }

    pub fn attach_surface(&mut self, surface: u32, texture: wgpu::Texture) {
        let state = self.store.data_mut();
        match state.gpu() {
            Ok(gpu) => gpu.attach_surface(surface, texture),
            Err(message) => {
                state.error.get_or_insert(message);
            }
        }
    }

    pub fn detach_surface(&mut self, surface: u32) {
        if let Some(gpu) = self.store.data_mut().device.ready_mut() {
            gpu.detach_surface(surface);
        }
    }

    pub fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        self.store.data().threads.stop();
        if !self.interrupted() {
            let _ = self.shutdown.call(&mut self.store, ());
        }
    }

    fn interrupted(&self) -> bool {
        self.store.data().threads.halts(true)
    }
}

fn call_constructors<T: 'static>(instance: &Instance, store: &mut Store<T>) -> Result<(), String> {
    let Ok(constructors) = instance.get_typed_func::<(), ()>(&mut *store, CONSTRUCTORS) else {
        return Ok(());
    };
    constructors
        .call(store, ())
        .map_err(|error| format!("the plugin could not run its constructors: {error}"))
}

fn initialize(instance: &Instance, store: &mut Store<State>) -> Result<(), String> {
    let Ok(initialize) = instance.get_typed_func::<(), ()>(&mut *store, INITIALIZE) else {
        return call_constructors(instance, store);
    };
    initialize
        .call(store, ())
        .map_err(|error| format!("the plugin could not initialize itself: {error}"))
}

fn memory_import(module: &Module) -> Option<(String, String, MemoryType)> {
    module.imports().find_map(|import| {
        let declared = import.ty().memory()?.clone();
        Some((import.module().to_owned(), import.name().to_owned(), declared))
    })
}

fn initialize_storage(
    runtime: &Instance,
    instance: &Instance,
    store: &mut Store<State>,
) -> Result<(), String> {
    let size = global(runtime, store, "__tls_size")?;
    let align = global(runtime, store, "__tls_align")?;
    let initialize: TypedFunc<(u32, u32), ()> = typed(instance, store, "plugin_initialize_tls")?;
    initialize
        .call(store, (size, align))
        .map_err(|error| format!("the plugin could not set up thread storage: {error}"))
}

fn global(instance: &Instance, store: &mut Store<State>, name: &str) -> Result<u32, String> {
    instance
        .get_global(&mut *store, name)
        .and_then(|global| global.get(store).i32())
        .map(|value| value as u32)
        .ok_or_else(|| format!("the plugin does not export {name}"))
}

fn module(engine: &Engine, bytes: &[u8]) -> Result<Module, String> {
    if Engine::detect_precompiled(bytes).is_some() {
        // unsafe(ffi)
        return unsafe { Module::deserialize(engine, bytes) }
            .map_err(|error| format!("the compiled plugin module could not be loaded: {error}"));
    }
    Module::new(engine, bytes)
        .map_err(|error| format!("the plugin module could not be compiled: {error}"))
}

fn sdk_module(engine: &Engine, identity: &str, bytes: &[u8]) -> Result<Module, String> {
    if Engine::detect_precompiled(bytes).is_none() {
        wasm_dylink::check_sdk(bytes, identity)?;
    }
    module(engine, bytes)
}

fn precompiled_beside(engine: &Engine, path: &Path) -> Option<Module> {
    let artifact = path.with_extension(PRECOMPILED_EXTENSION);
    if !artifact.is_file() {
        return None;
    }
    // unsafe(ffi)
    match unsafe { Module::deserialize_file(engine, &artifact) } {
        Ok(module) => Some(module),
        Err(error) => {
            eprintln!(
                "{} could not be used, compiling the plugin instead: {error}",
                artifact.display()
            );
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flavor {
    Plugin,
    Pure,
}

type Engines = Vec<(Flavor, Option<PathBuf>, Engine, Sdks)>;

fn engine(cache: Option<&Path>, flavor: Flavor) -> Result<(Engine, Sdks), String> {
    static ENGINES: Mutex<Engines> = Mutex::new(Vec::new());
    let mut engines = ENGINES
        .lock()
        .map_err(|_| "the wasm engines were poisoned".to_owned())?;
    if let Some((_, _, engine, sdks)) = engines
        .iter()
        .find(|(kind, directory, _, _)| *kind == flavor && directory.as_deref() == cache)
    {
        return Ok((engine.clone(), Arc::clone(sdks)));
    }
    let mut config = configuration(flavor);
    if let Some(directory) = cache {
        config.cache(Some(compilation_cache(directory)?));
    }
    let engine = Engine::new(&config)
        .map_err(|error| format!("the wasm engine could not start: {error}"))?;
    let sdks = Sdks::default();
    engines.push((
        flavor,
        cache.map(Path::to_path_buf),
        engine.clone(),
        Arc::clone(&sdks),
    ));
    Ok((engine, sdks))
}

fn configuration(flavor: Flavor) -> Config {
    let mut config = Config::new();
    match flavor {
        Flavor::Plugin => {
            config.wasm_threads(true);
            config.shared_memory(true);
            config.wasm_exceptions(true);
        }
        Flavor::Pure => {
            config.wasm_threads(false);
            config.shared_memory(false);
            config.wasm_relaxed_simd(false);
        }
    }
    config.wasm_bulk_memory(true);
    config.epoch_interruption(true);
    config.wasm_backtrace_max_frames(NonZeroUsize::new(BACKTRACE_FRAMES));
    if std::env::var_os("BE3_PERF_MAP").is_some() {
        config.profiler(wasmtime::ProfilingStrategy::PerfMap);
    }
    config
}

fn compilation_cache(directory: &Path) -> Result<Cache, String> {
    std::fs::create_dir_all(directory).map_err(|error| {
        format!(
            "the plugin cache directory {} could not be made: {error}",
            directory.display()
        )
    })?;
    let directory = directory.canonicalize().map_err(|error| {
        format!(
            "the plugin cache directory {} could not be resolved: {error}",
            directory.display()
        )
    })?;
    let mut configuration = CacheConfig::new();
    configuration.with_directory(directory);
    Cache::new(configuration)
        .map_err(|error| format!("the plugin cache could not be opened: {error}"))
}

fn shared_memory(engine: &Engine, module: &Module) -> Result<SharedMemory, String> {
    let declared = module
        .imports()
        .find(|import| import.module() == "env" && import.name() == "memory")
        .and_then(|import| import.ty().memory().cloned())
        .ok_or_else(|| "the plugin does not import env.memory".to_owned())?;
    if !declared.is_shared() {
        return Err("the plugin memory is not shared".to_owned());
    }
    SharedMemory::new(engine, declared)
        .map_err(|error| format!("the plugin memory could not be created: {error}"))
}

fn typed<Parameters, Results>(
    instance: &Instance,
    store: &mut Store<State>,
    name: &str,
) -> Result<TypedFunc<Parameters, Results>, String>
where
    Parameters: wasmtime::WasmParams,
    Results: wasmtime::WasmResults,
{
    instance
        .get_typed_func(store, name)
        .map_err(|error| format!("the plugin has no usable {name} export: {error}"))
}
