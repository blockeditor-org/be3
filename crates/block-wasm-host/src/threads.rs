use std::{
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    time::Duration,
};

use block_gpu_abi::embed::device as gpu;
use block_guest_abi as guest;
use wasmtime::{Caller, Engine, Error, Instance, Linker, SharedMemory, Store, TypedFunc};
use wasmtime_wasi::{WasiCtxBuilder, p1};

use crate::dylink::{self, Layout, Program};
use crate::interrupt::{self, Interrupt};
use crate::state::Threaded;
use crate::wake::Wake;

const MODULE: &str = "wasi";
const SPAWN: &str = "thread-spawn";
const START: &str = "wasi_thread_start";
const REFUSED: i32 = -1;
const UNSHARED: &str = "the plugin spawned a thread, which needs a shared memory it imports";
const REAPING_PASSES: u32 = 8;
const REAPING_WAIT: Duration = Duration::from_millis(50);

pub(crate) trait Spawns {
    fn spawner(&self) -> &Arc<Spawner>;
}

pub(crate) struct Spawner {
    engine: Engine,
    program: Program,
    layout: OnceLock<Layout>,
    memory: Option<SharedMemory>,
    wake: Arc<Wake>,
    interrupt: Arc<Interrupt>,
    next: AtomicI32,
    stopped: AtomicBool,
    halted: AtomicBool,
    reaping: AtomicBool,
    alive: Mutex<usize>,
    ended: Condvar,
    failures: Mutex<Vec<String>>,
}

impl Spawner {
    pub(crate) fn new(
        engine: Engine,
        program: Program,
        memory: Option<SharedMemory>,
        wake: Arc<Wake>,
        interrupt: Arc<Interrupt>,
    ) -> Arc<Self> {
        let spawner = Arc::new(Self {
            engine,
            program,
            layout: OnceLock::new(),
            memory,
            wake,
            interrupt,
            next: AtomicI32::new(1),
            stopped: AtomicBool::new(false),
            halted: AtomicBool::new(false),
            reaping: AtomicBool::new(false),
            alive: Mutex::new(1),
            ended: Condvar::new(),
            failures: Mutex::new(Vec::new()),
        });
        spawner.interrupt.watch(&spawner);
        spawner
    }

    pub(crate) fn linked(&self, layout: Layout) {
        let _ = self.layout.set(layout);
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    pub(crate) fn halts(&self, main: bool) -> bool {
        self.interrupt.interrupted() || (!main && self.halted.load(Ordering::SeqCst))
    }

    pub(crate) fn release(self: &Arc<Self>) {
        self.stop();
        self.halted.store(true, Ordering::SeqCst);
        self.interrupt.tick();
        self.end();
        self.reap();
    }

    pub(crate) fn reap(self: &Arc<Self>) {
        if self.living() == 0 || self.reaping.swap(true, Ordering::SeqCst) {
            return;
        }
        let spawner = Arc::clone(self);
        let reaper = std::thread::Builder::new()
            .name("plugin reaper".to_owned())
            .spawn(move || {
                for _ in 0..REAPING_PASSES {
                    if spawner.await_ended(REAPING_WAIT) {
                        break;
                    }
                    spawner.notify_every_waiter();
                }
                spawner.reaping.store(false, Ordering::SeqCst);
            });
        if reaper.is_err() {
            self.reaping.store(false, Ordering::SeqCst);
        }
    }

    fn notify_every_waiter(&self) {
        let Some(memory) = &self.memory else {
            return;
        };
        let size = memory.data_size() as u64;
        for address in (0..size).step_by(4) {
            let _ = memory.atomic_notify(address, u32::MAX);
        }
    }

    fn living(&self) -> usize {
        self.alive.lock().map_or(0, |alive| *alive)
    }

    pub(crate) fn await_ended(&self, within: Duration) -> bool {
        let Ok(alive) = self.alive.lock() else {
            return false;
        };
        self.ended
            .wait_timeout_while(alive, within, |alive| *alive > 0)
            .is_ok_and(|(alive, _)| *alive == 0)
    }

    fn begin(&self) {
        if let Ok(mut alive) = self.alive.lock() {
            *alive += 1;
        }
    }

    fn end(&self) {
        if let Ok(mut alive) = self.alive.lock() {
            *alive = alive.saturating_sub(1);
            if *alive == 0 {
                self.ended.notify_all();
            }
        }
    }

    pub(crate) fn take_failure(&self) -> Option<String> {
        let mut failures = self.failures.lock().ok()?;
        match failures.is_empty() {
            true => None,
            false => Some(failures.remove(0)),
        }
    }

    fn spawn(self: &Arc<Self>, argument: i32) -> i32 {
        if self.stopped.load(Ordering::Acquire) || self.halts(false) {
            return REFUSED;
        }
        let Some(memory) = &self.memory else {
            self.fail(UNSHARED.to_owned());
            return REFUSED;
        };
        let thread_locals = match &self.program {
            Program::Whole(_) => None,
            Program::Linked { .. } => match dylink::spawned_thread_locals(memory, argument) {
                Ok(base) => Some(base),
                Err(failure) => {
                    self.fail(failure);
                    return REFUSED;
                }
            },
        };
        let thread = self.next.fetch_add(1, Ordering::Relaxed);
        let spawner = Arc::clone(self);
        self.begin();
        let spawned = std::thread::Builder::new()
            .name(format!("plugin thread {thread}"))
            .spawn(move || {
                let entered = spawner.enter(thread, argument, thread_locals);
                if let Err(failure) = entered
                    && !spawner.halts(false)
                {
                    spawner.fail(failure);
                }
                spawner.end();
            });
        match spawned {
            Ok(_) => thread,
            Err(error) => {
                self.end();
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
        interrupt::watch(&mut store, false);
        let mut linker: Linker<Threaded> = Linker::new(&self.engine);
        p1::add_to_linker_sync(&mut linker, |state: &mut Threaded| &mut state.wasi)
            .map_err(|error| format!("wasi could not be linked for a plugin thread: {error}"))?;
        let memory = self.memory.clone().ok_or(UNSHARED)?;
        for import in self.program.first().imports() {
            if import.ty().memory().is_some() {
                linker
                    .define(&store, import.module(), import.name(), memory.clone())
                    .map_err(|error| format!("a plugin thread could not share memory: {error}"))?;
            }
        }
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
        let memory = self.memory.as_ref().ok_or(UNSHARED)?;
        let instance = dylink::instantiate(store, linker, sdk, plugin, layout)?;
        dylink::place_thread_locals(store, &instance, memory, layout, base)
    }

    fn refuse_host_calls(&self, linker: &mut Linker<Threaded>) -> Result<(), String> {
        linker.allow_shadowing(true);
        for import in self
            .program
            .modules()
            .into_iter()
            .flat_map(|module| module.imports())
        {
            if !matches!(import.module(), gpu::MODULE | guest::HOST_MODULE) {
                continue;
            }
            if import.module() == guest::HOST_MODULE
                && matches!(import.name(), guest::HOST_WAKE | guest::HOST_PANIC)
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
        crate::state::read(self.memory.as_ref().ok_or(UNSHARED)?, pointer, length)
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
