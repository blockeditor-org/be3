//! `thread-spawn`: each app thread is an OS thread with an isolate of its own, which instantiates
//! the app's modules (compiled once, so nothing is compiled again) against the same shared memory,
//! handed over as a serialized `WebAssembly.Memory` whose buffer is the one backing store.

use std::{
    sync::{
        Arc, Condvar, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicI32, Ordering},
    },
    time::Duration,
};

use v8::{ValueDeserializerHelper, ValueSerializerHelper};

use crate::dylink::{self, Linked, Program};
use crate::engine::{self, Defined};
use crate::instance::{self, Answers, Role};
use crate::interrupt::Interrupt;
use crate::state::{Device, Memory, State, state_of};
use crate::wake::Wake;
use crate::wasi::Wasi;

const MODULE: &str = "wasi";
const SPAWN: &str = "thread-spawn";
const START: &str = "wasi_thread_start";
const REFUSED: i32 = -1;
const UNSHARED: &str = "the app spawned a thread, which needs a shared memory it imports";

/// A shared memory as another isolate takes it: the `WebAssembly.Memory` serialized once, and
/// the backing store its buffer is.
pub(crate) struct Share {
    serialized: Vec<u8>,
    pub(crate) store: v8::SharedRef<v8::BackingStore>,
}

struct Serializing;

impl v8::ValueSerializerImpl for Serializing {
    fn throw_data_clone_error<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        message: v8::Local<'s, v8::String>,
    ) {
        let error = v8::Exception::error(scope, message);
        scope.throw_exception(error);
    }

    fn get_shared_array_buffer_id<'s>(
        &self,
        _: &mut v8::PinScope<'s, '_>,
        _: v8::Local<'s, v8::SharedArrayBuffer>,
    ) -> Option<u32> {
        Some(0)
    }
}

struct Deserializing(v8::SharedRef<v8::BackingStore>);

impl v8::ValueDeserializerImpl for Deserializing {
    fn get_shared_array_buffer_from_id<'s>(
        &self,
        scope: &mut v8::PinScope<'s, '_>,
        _: u32,
    ) -> Option<v8::Local<'s, v8::SharedArrayBuffer>> {
        Some(v8::SharedArrayBuffer::with_backing_store(scope, &self.0))
    }
}

impl Share {
    pub(crate) fn of(
        scope: &mut v8::PinScope<'_, '_>,
        memory: v8::Local<v8::Object>,
    ) -> Result<Self, String> {
        let buffer: v8::Local<v8::SharedArrayBuffer> = engine::get(scope, memory, "buffer")
            .and_then(|buffer| buffer.try_into().ok())
            .ok_or("the app memory is not shared")?;
        let store = buffer.get_backing_store();
        let serializer = v8::ValueSerializer::new(scope, Box::new(Serializing));
        serializer.write_header();
        let context = scope.get_current_context();
        serializer
            .write_value(context, memory.into())
            .filter(|written| *written)
            .ok_or("the app memory could not be handed to a thread")?;
        Ok(Self {
            serialized: serializer.release(),
            store,
        })
    }

    /// The memory, as a `WebAssembly.Memory` of this isolate.
    fn take<'s>(&self, scope: &mut v8::PinScope<'s, '_>) -> Result<v8::Local<'s, v8::Object>, String> {
        let deserializer = v8::ValueDeserializer::new(
            scope,
            Box::new(Deserializing(self.store.clone())),
            &self.serialized,
        );
        let context = scope.get_current_context();
        deserializer
            .read_header(context)
            .filter(|read| *read)
            .and_then(|_| deserializer.read_value(context))
            .and_then(|memory| memory.to_object(scope))
            .ok_or_else(|| "an app thread could not take the app memory".to_owned())
    }
}

pub(crate) struct Spawner {
    program: Program,
    linked: OnceLock<Arc<Linked>>,
    memory: Option<Share>,
    wake: Arc<Wake>,
    wasi: Arc<Wasi>,
    interrupt: Arc<Interrupt>,
    next: AtomicI32,
    stopped: AtomicBool,
    halted: AtomicBool,
    alive: Mutex<usize>,
    ended: Condvar,
    failures: Mutex<Vec<String>>,
    /// The isolates of the app (thread 0) and of its threads, to terminate.
    isolates: Mutex<Vec<(i32, v8::IsolateHandle)>>,
}

impl Spawner {
    pub(crate) fn new(
        program: Program,
        memory: Option<Share>,
        wake: Arc<Wake>,
        wasi: Arc<Wasi>,
        interrupt: Arc<Interrupt>,
        main: v8::IsolateHandle,
    ) -> Arc<Self> {
        let spawner = Arc::new(Self {
            program,
            linked: OnceLock::new(),
            memory,
            wake,
            wasi,
            interrupt,
            next: AtomicI32::new(1),
            stopped: AtomicBool::new(false),
            halted: AtomicBool::new(false),
            alive: Mutex::new(1),
            ended: Condvar::new(),
            failures: Mutex::new(Vec::new()),
            isolates: Mutex::new(vec![(0, main)]),
        });
        spawner.interrupt.watch(&spawner);
        spawner
    }

    pub(crate) fn linked(&self, linked: Arc<Linked>) {
        let _ = self.linked.set(linked);
    }

    pub(crate) fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    pub(crate) fn halts(&self, main: bool) -> bool {
        self.interrupt.interrupted() || (!main && self.halted.load(Ordering::SeqCst))
    }

    /// Terminates the threads' isolates, and the app's own with `main`.
    pub(crate) fn terminate(&self, main: bool) {
        if let Ok(isolates) = self.isolates.lock() {
            for (thread, isolate) in isolates.iter() {
                if main || *thread != 0 {
                    isolate.terminate_execution();
                }
            }
        }
    }

    /// The app is gone: its threads stop.
    pub(crate) fn release(&self) {
        self.stop();
        self.halted.store(true, Ordering::SeqCst);
        self.terminate(false);
        self.end();
    }

    #[cfg(test)]
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

    pub(crate) fn share(&self) -> Option<&Share> {
        self.memory.as_ref()
    }

    /// Starts a thread at `wasi_thread_start(thread, argument)`. `grow` grows the memory by some
    /// pages on the spawning isolate, for the new thread's thread-local blocks.
    fn spawn(
        self: &Arc<Self>,
        argument: i32,
        grow: &mut dyn FnMut(u32) -> Result<u32, String>,
    ) -> i32 {
        if self.stopped.load(Ordering::Acquire) || self.halts(false) {
            return REFUSED;
        }
        let Some(memory) = &self.memory else {
            self.fail(UNSHARED.to_owned());
            return REFUSED;
        };
        let thread_locals = match &self.program {
            Program::Whole(_) => None,
            Program::Linked(_) => match self.blocks(&memory.store, argument, grow) {
                Ok(thread) => Some(thread),
                Err(failure) => {
                    self.fail(failure);
                    return REFUSED;
                }
            },
        };
        let blocks = thread_locals.as_ref().map(|thread| thread.blocks);
        let thread = self.next.fetch_add(1, Ordering::Relaxed);
        let spawner = Arc::clone(self);
        self.begin();
        let spawned = std::thread::Builder::new()
            .name(format!("app thread {thread}"))
            .spawn(move || {
                let entered = spawner.enter(thread, argument, thread_locals);
                if let Err(failure) = entered
                    && !spawner.halts(false)
                {
                    spawner.fail(failure);
                }
                spawner.free(blocks);
                spawner.end();
            });
        match spawned {
            Ok(_) => thread,
            Err(error) => {
                self.free(blocks);
                self.end();
                self.fail(format!("an app thread could not start: {error}"));
                REFUSED
            }
        }
    }

    fn enter(
        self: &Arc<Self>,
        thread: i32,
        argument: i32,
        thread_locals: Option<dylink::Thread>,
    ) -> Result<(), String> {
        self.memory.as_ref().ok_or(UNSHARED)?;
        let engine::Isolate {
            shared: isolate,
            context,
            exhausted,
        } = engine::isolate();
        let handle = isolate.thread_safe_handle();
        if let Ok(mut isolates) = self.isolates.lock() {
            isolates.push((thread, handle.clone()));
        }
        let entered = match self.halts(false) {
            true => Ok(()),
            false => self.on(&isolate, &context, handle, thread, argument, thread_locals),
        };
        if let Ok(mut isolates) = self.isolates.lock() {
            isolates.retain(|(running, _)| *running != thread);
        }
        drop(context);
        drop(isolate);
        match exhausted.reached() {
            true => Err(format!("an app thread: {}", engine::OUT_OF_MEMORY)),
            false => entered,
        }
    }

    fn on(
        self: &Arc<Self>,
        isolate: &v8::SharedIsolate,
        context: &v8::Global<v8::Context>,
        handle: v8::IsolateHandle,
        thread: i32,
        argument: i32,
        thread_locals: Option<dylink::Thread>,
    ) -> Result<(), String> {
        let share = self.memory.as_ref().ok_or(UNSHARED)?;
        let state = Box::into_raw(Box::new(State {
            memory: Memory::Shared(share.store.clone()),
            memory_object: None,
            device: Device::Unavailable("an app thread has no gpu".to_owned()),
            imports: Default::default(),
            error: None,
            fatal: None,
            panicked: None,
            returned: Vec::new(),
            transport: Default::default(),
            threads: Arc::clone(self),
            wake: Arc::clone(&self.wake),
            wasi: Arc::clone(&self.wasi),
            gpu_time: Duration::ZERO,
            submit_time: Duration::ZERO,
            isolate: handle,
        }));
        let entered = {
            let mut locker = isolate.lock();
            v8::scope!(let scope, &mut *locker);
            let context = v8::Local::new(scope, context);
            let scope = &mut v8::ContextScope::new(scope, context);
            v8::tc_scope!(let caught, scope);
            let run = self.run(caught, state, thread, argument, thread_locals.as_ref());
            match run {
                Ok(()) => Ok(()),
                Err(failure) => {
                    // the thread's isolate has stopped running the guest.
                    // unsafe(memory)
                    let state = unsafe { &mut *state };
                    Err(match (state.panicked.take(), state.fatal.take()) {
                        (Some(panicked), _) => panicked,
                        (None, Some(fatal)) => fatal,
                        (None, None) if caught.has_caught() => {
                            format!("{failure}: {}", crate::engine::thrown!(caught))
                        }
                        (None, None) => failure,
                    })
                }
            }
        };
        // the isolate no longer runs, and nothing it holds reaches the State again.
        // unsafe(memory)
        drop(unsafe { Box::from_raw(state) });
        entered
    }

    fn run<'s>(
        self: &Arc<Self>,
        scope: &mut v8::PinScope<'s, '_>,
        state: *mut State,
        thread: i32,
        argument: i32,
        thread_locals: Option<&dylink::Thread>,
    ) -> Result<(), String> {
        let share = self.memory.as_ref().ok_or(UNSHARED)?;
        let memory = share.take(scope)?;
        // the guest has not started, so nothing else holds the State.
        // unsafe(memory)
        unsafe { (*state).memory_object = Some(v8::Global::new(scope, memory)) };
        let data = v8::External::new(scope, state.cast()).into();
        let modules = self.program.modules();
        let answers = Answers {
            defined: instance::host_functions(scope, data, Role::Thread, modules)?,
            memory: Some(memory),
            pure: false,
        };
        let root = instance::instantiate_answered(scope, self.program.first(), &answers)
            .map_err(|error| format!("an app thread could not be instantiated: {error}"))?;
        if let (Program::Linked(modules), Some(thread)) = (&self.program, thread_locals) {
            let linked = self
                .linked
                .get()
                .ok_or("an app thread started before its app was linked")?;
            dylink::link(scope, &answers, modules, linked, root, thread)?;
        }
        let start = instance::function(scope, root, START)
            .ok_or_else(|| format!("the app has no usable {START} export"))?;
        let receiver = v8::undefined(scope).into();
        let arguments = [
            v8::Integer::new(scope, thread).into(),
            v8::Integer::new(scope, argument).into(),
        ];
        start
            .call(scope, receiver, &arguments)
            .map(|_| ())
            .ok_or_else(|| "an app thread trapped".to_owned())
    }

    fn blocks(
        &self,
        store: &v8::SharedRef<v8::BackingStore>,
        argument: i32,
        grow: &mut dyn FnMut(u32) -> Result<u32, String>,
    ) -> Result<dylink::Thread, String> {
        let linked = self
            .linked
            .get()
            .ok_or("an app thread was spawned before its app was linked")?;
        Ok(dylink::Thread {
            blocks: linked.take(store, grow)?,
            root_block: dylink::spawned_thread_locals(store, argument)?,
            main: false,
        })
    }

    fn free(&self, blocks: Option<u32>) {
        if let (Some(blocks), Some(linked)) = (blocks, self.linked.get()) {
            linked.give(blocks);
        }
    }

    fn fail(&self, failure: String) {
        eprintln!("{failure}");
        if let Ok(mut failures) = self.failures.lock() {
            failures.push(failure);
        }
        self.wake.wake();
    }
}

/// Grows the memory of the isolate `scope` is on by `pages`, as `memory.grow` does, answering the
/// pages it had.
pub(crate) fn grow(
    scope: &mut v8::PinScope<'_, '_>,
    memory: v8::Local<v8::Object>,
    pages: u32,
) -> Result<u32, String> {
    let pages = engine::number(scope, f64::from(pages));
    engine::call(scope, memory, "grow", &[pages])
        .and_then(|had| had.uint32_value(scope))
        .ok_or_else(|| "the app memory could not grow".to_owned())
}

fn thread_spawn(
    scope: &mut v8::PinScope,
    arguments: v8::FunctionCallbackArguments,
    mut returned: v8::ReturnValue<v8::Value>,
) {
    // the function's data is its State (state_of).
    // unsafe(memory)
    let state = unsafe { state_of(arguments.data()) };
    let argument = arguments.get(0).int32_value(scope).unwrap_or(0);
    let spawner = Arc::clone(&state.threads);
    let memory = state.memory_object.as_ref().map(|memory| v8::Local::new(scope, memory));
    let mut grow = |pages: u32| match memory {
        Some(memory) => grow(scope, memory, pages),
        None => Err(UNSHARED.to_owned()),
    };
    match crate::gpu::contained(|| spawner.spawn(argument, &mut grow)) {
        Ok(thread) => returned.set_int32(thread),
        Err(panic) => {
            crate::gpu::throw(scope, &panic);
            state.fatal.get_or_insert(panic);
        }
    }
}

pub(crate) fn define<'s>(
    scope: &mut v8::PinScope<'s, '_>,
    defined: &mut Defined<'s>,
    data: v8::Local<'s, v8::Value>,
) -> Result<(), String> {
    engine::function(scope, defined, MODULE, SPAWN, data, thread_spawn)
}
