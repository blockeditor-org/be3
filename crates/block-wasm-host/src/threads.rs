use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use block_gpu_abi as abi;
use wasmtime::{Caller, Engine, Error, Instance, Linker, SharedMemory, Store, TypedFunc};
use wasmtime_wasi::{WasiCtxBuilder, p1};

use crate::dylink::{self, Layout, Program};
use crate::state::Threaded;
use crate::wake::Wake;

const MODULE: &str = "wasi";
const SPAWN: &str = "thread-spawn";
const START: &str = "wasi_thread_start";
const REFUSED: i32 = -1;

pub(crate) trait Spawns {
    fn spawner(&self) -> &Arc<Spawner>;
}

pub(crate) struct Spawner {
    engine: Engine,
    program: Program,
    layout: OnceLock<Layout>,
    memory: SharedMemory,
    wake: Arc<Wake>,
    next: AtomicI32,
    stopped: AtomicBool,
    failures: Mutex<Vec<String>>,
}

impl Spawner {
    pub(crate) fn new(
        engine: Engine,
        program: Program,
        memory: SharedMemory,
        wake: Arc<Wake>,
    ) -> Arc<Self> {
        Arc::new(Self {
            engine,
            program,
            layout: OnceLock::new(),
            memory,
            wake,
            next: AtomicI32::new(1),
            stopped: AtomicBool::new(false),
            failures: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn linked(&self, layout: Layout) {
        let _ = self.layout.set(layout);
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    pub(crate) fn take_failure(&self) -> Option<String> {
        let mut failures = self.failures.lock().ok()?;
        match failures.is_empty() {
            true => None,
            false => Some(failures.remove(0)),
        }
    }

    fn spawn(self: &Arc<Self>, argument: i32) -> i32 {
        if self.stopped.load(Ordering::Acquire) {
            return REFUSED;
        }
        let thread_locals = match &self.program {
            Program::Whole(_) => None,
            Program::Linked { .. } => match dylink::spawned_thread_locals(&self.memory, argument) {
                Ok(base) => Some(base),
                Err(failure) => {
                    self.fail(failure);
                    return REFUSED;
                }
            },
        };
        let thread = self.next.fetch_add(1, Ordering::Relaxed);
        let spawner = Arc::clone(self);
        let spawned = std::thread::Builder::new()
            .name(format!("plugin thread {thread}"))
            .spawn(move || {
                if let Err(failure) = spawner.enter(thread, argument, thread_locals) {
                    spawner.fail(failure);
                }
            });
        match spawned {
            Ok(_) => thread,
            Err(error) => {
                self.fail(format!("a plugin thread could not start: {error}"));
                REFUSED
            }
        }
    }

    fn enter(
        self: &Arc<Self>,
        thread: i32,
        argument: i32,
        thread_locals: Option<u32>,
    ) -> Result<(), String> {
        let wasi = WasiCtxBuilder::new().inherit_stderr().build_p1();
        let state = Threaded {
            wasi,
            threads: Arc::clone(self),
            wake: Arc::clone(&self.wake),
        };
        let mut store = Store::new(&self.engine, state);
        let mut linker: Linker<Threaded> = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut Threaded| &mut state.wasi)
            .map_err(|error| format!("wasi could not be linked for a plugin thread: {error}"))?;
        linker
            .define(&store, "env", "memory", self.memory.clone())
            .map_err(|error| format!("a plugin thread could not share memory: {error}"))?;
        link(&mut linker)?;
        crate::wake::link(&mut linker)?;
        crate::panic::link(&mut linker)?;
        self.refuse_host_calls(&mut linker)?;
        linker
            .define_unknown_imports_as_traps(self.program.first())
            .map_err(|error| format!("a plugin thread could not stub its imports: {error}"))?;
        let instance = linker
            .instantiate(&mut store, self.program.first())
            .map_err(|error| format!("a plugin thread could not be instantiated: {error}"))?;
        self.link_plugin(&mut store, &linker, &instance, thread_locals)?;
        let start: TypedFunc<(i32, i32), ()> = instance
            .get_typed_func(&mut store, START)
            .map_err(|error| format!("the plugin has no usable {START} export: {error}"))?;
        start.call(&mut store, (thread, argument)).map_err(|error| {
            crate::panic::message(&error)
                .unwrap_or_else(|| format!("a plugin thread trapped: {error:?}"))
        })
    }

    fn link_plugin(
        &self,
        store: &mut Store<Threaded>,
        linker: &Linker<Threaded>,
        sdk: &Instance,
        thread_locals: Option<u32>,
    ) -> Result<(), String> {
        let (Program::Linked { plugin, .. }, Some(base)) = (&self.program, thread_locals) else {
            return Ok(());
        };
        let layout = self
            .layout
            .get()
            .ok_or("a plugin thread started before its plugin was linked")?;
        let instance = dylink::instantiate(store, linker, sdk, plugin, layout)?;
        dylink::place_thread_locals(store, &instance, &self.memory, layout, base)
    }

    fn refuse_host_calls(&self, linker: &mut Linker<Threaded>) -> Result<(), String> {
        linker.allow_shadowing(true);
        for import in self
            .program
            .modules()
            .into_iter()
            .flat_map(|module| module.imports())
        {
            if !matches!(import.module(), abi::GPU_MODULE | abi::HOST_MODULE) {
                continue;
            }
            if import.module() == abi::HOST_MODULE
                && matches!(import.name(), abi::HOST_WAKE | abi::HOST_PANIC)
            {
                continue;
            }
            let Some(signature) = import.ty().func().cloned() else {
                continue;
            };
            let refusal = format!(
                "a plugin thread called {}::{}, which only its main thread may do",
                import.module(),
                import.name()
            );
            linker
                .func_new(import.module(), import.name(), signature, move |_, _, _| {
                    Err(Error::msg(refusal.clone()))
                })
                .map_err(|error| format!("a host call could not be refused: {error}"))?;
        }
        Ok(())
    }

    pub(crate) fn read(&self, pointer: u32, length: u32) -> Result<Vec<u8>, String> {
        crate::state::read(&self.memory, pointer, length)
    }

    fn fail(&self, failure: String) {
        eprintln!("{failure}");
        if let Ok(mut failures) = self.failures.lock() {
            failures.push(failure);
        }
        self.wake.wake();
    }
}

pub(crate) fn link<T: Spawns + 'static>(linker: &mut Linker<T>) -> Result<(), String> {
    linker
        .func_wrap(
            MODULE,
            SPAWN,
            |caller: Caller<'_, T>, argument: i32| -> i32 {
                caller.data().spawner().spawn(argument)
            },
        )
        .map(|_| ())
        .map_err(|error| format!("{SPAWN} could not be linked: {error}"))
}
