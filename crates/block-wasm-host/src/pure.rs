use std::{
    collections::VecDeque,
    convert::Infallible,
    path::Path,
    sync::{Arc, atomic::AtomicBool},
    time::Instant,
};

use block_guest_abi::{self as abi, direct};
use block_guest_host::{
    Produced, StepTime, Wake as HostWake,
    thread::{self, Runnable, Spawn, Threaded},
};
use wasmtime::{
    Caller, Engine, Extern, Instance, Linker, Memory, Module, Store, StoreLimits,
    StoreLimitsBuilder, TypedFunc, UpdateDeadline,
};

use crate::{
    Flavor, INTERRUPTED, PRECOMPILED_EXTENSION, call_constructors, engine,
    interrupt::{Interrupt, Interrupter},
    module, panic,
    wake::{Wake, Wakes},
};

pub const MEMORY_LIMIT: usize = 1 << 30;

const MEMORY: &str = "memory";

#[derive(Clone)]
pub struct PureHost {
    engine: Engine,
    interrupt: Arc<Interrupt>,
    memory_limit: usize,
    imports: Vec<(String, String)>,
}

impl PureHost {
    pub fn new(cache: Option<&Path>) -> Result<Self, String> {
        let (engine, _) = engine(cache, Flavor::Pure)?;
        Ok(Self {
            interrupt: Interrupt::new(engine.clone()),
            engine,
            memory_limit: MEMORY_LIMIT,
            imports: Vec::new(),
        })
    }

    pub fn with_imports(mut self, imports: &[(&str, &str)]) -> Self {
        self.imports = imports
            .iter()
            .map(|(module, name)| ((*module).to_owned(), (*name).to_owned()))
            .collect();
        self
    }

    pub fn with_memory_limit(mut self, bytes: usize) -> Self {
        self.memory_limit = bytes;
        self
    }

    pub fn interrupter(&self) -> Interrupter {
        Interrupter::new(&self.interrupt)
    }

    pub fn load(&self, path: &Path) -> Result<PureGuest, String> {
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
    ) -> Result<PureGuest, String> {
        let module = match self.precompiled(entry, &read) {
            Some(module) => module,
            None => module(&self.engine, &read(entry)?)?,
        };
        PureGuest::new(self, &module)
    }

    pub fn load_bytes(&self, bytes: &[u8]) -> Result<PureGuest, String> {
        PureGuest::new(self, &module(&self.engine, bytes)?)
    }

    pub fn spawn(
        self,
        name: String,
        load: impl FnOnce(&PureHost) -> Result<PureGuest, String> + Send + 'static,
        wake: HostWake,
        waiting: Arc<AtomicBool>,
    ) -> Result<Threaded<Infallible>, String> {
        let interrupter = self.interrupter();
        thread::spawn(
            Spawn {
                name,
                wake,
                waiting,
                interrupt: Box::new(move || interrupter.interrupt()),
                contain: thread::catch,
            },
            move |wake| {
                let guest = load(&self)?;
                guest.on_wake(move || wake());
                Ok(guest)
            },
        )
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
                eprintln!("{artifact} could not be used, compiling the guest instead: {error}");
                None
            }
        }
    }
}

struct PureState {
    inbox: VecDeque<Vec<u8>>,
    outbox: Vec<Vec<u8>>,
    wake: Arc<Wake>,
    limits: StoreLimits,
    interrupt: Arc<Interrupt>,
}

impl Wakes for PureState {
    fn wake(&self) -> &Arc<Wake> {
        &self.wake
    }
}

pub struct PureGuest {
    store: Store<PureState>,
    instance: Instance,
    start: TypedFunc<(), ()>,
    step: TypedFunc<(), ()>,
    shutdown: TypedFunc<(), ()>,
    wake: Arc<Wake>,
    stopped: bool,
}

impl PureGuest {
    fn new(host: &PureHost, module: &Module) -> Result<Self, String> {
        if host.interrupt.interrupted() {
            return Err(INTERRUPTED.to_owned());
        }
        let wake: Arc<Wake> = Arc::default();
        let state = PureState {
            inbox: VecDeque::new(),
            outbox: Vec::new(),
            wake: Arc::clone(&wake),
            limits: StoreLimitsBuilder::new()
                .memory_size(host.memory_limit)
                .trap_on_grow_failure(true)
                .build(),
            interrupt: Arc::clone(&host.interrupt),
        };
        let mut store = Store::new(&host.engine, state);
        store.limiter(|state| &mut state.limits);
        store.epoch_deadline_callback(|context| {
            Ok(match context.data().interrupt.interrupted() {
                true => UpdateDeadline::Interrupt,
                false => UpdateDeadline::Continue(1),
            })
        });
        store.set_epoch_deadline(1);
        let mut linker: Linker<PureState> = Linker::new(&host.engine);
        link(&mut linker)?;
        link_direct(&mut linker, &host.imports)?;
        crate::wake::link(&mut linker)?;
        let instance = linker
            .instantiate(&mut store, module)
            .map_err(|error| format!("the guest could not be instantiated: {error}"))?;
        call_constructors(&instance, &mut store)?;
        let typed = |store: &mut Store<PureState>, name: &str| {
            instance
                .get_typed_func::<(), ()>(store, name)
                .map_err(|error| format!("the guest has no usable {name} export: {error}"))
        };
        Ok(Self {
            start: typed(&mut store, abi::START)?,
            step: typed(&mut store, abi::STEP)?,
            shutdown: typed(&mut store, abi::SHUTDOWN)?,
            store,
            instance,
            wake,
            stopped: false,
        })
    }

    pub fn start(&mut self) -> Result<(), String> {
        self.call(self.start.clone())
    }

    pub fn step(&mut self) -> Result<(), String> {
        self.call(self.step.clone())
    }

    pub fn send(&mut self, frame: Vec<u8>) {
        self.store.data_mut().inbox.push_back(frame);
    }

    pub fn deliver(&mut self, items: Vec<Vec<u8>>) -> Result<(), String> {
        for item in items {
            match direct::decode(&item) {
                Some(direct::Item::Call(call)) => {
                    if !self.store.data().inbox.is_empty() {
                        self.step()?;
                    }
                    let name = call.name.to_owned();
                    let returned = self.call_export(&name, call.parent, call.args)?;
                    if call.answered {
                        self.store
                            .data_mut()
                            .outbox
                            .push(direct::returned(&returned));
                    }
                }
                Some(direct::Item::Frame(_)) => self.send(item),
                Some(direct::Item::Returned(_)) | None => {
                    return Err("the host sent the guest an item it cannot take".to_owned());
                }
            }
        }
        Ok(())
    }

    fn call_export(
        &mut self,
        name: &str,
        parent: Option<u64>,
        args: &[u8],
    ) -> Result<Vec<u8>, String> {
        let alloc = self
            .instance
            .get_typed_func::<u32, u32>(&mut self.store, direct::ALLOC)
            .map_err(|error| format!("the guest has no {}: {error}", direct::ALLOC))?;
        let export = self
            .instance
            .get_typed_func::<(i32, i64, u32, u32), i64>(&mut self.store, name)
            .map_err(|error| format!("the guest does not export {name}: {error}"))?;
        let failed = |error: wasmtime::Error| format!("{name} trapped: {error:?}");
        let pointer = alloc
            .call(&mut self.store, args.len() as u32)
            .map_err(failed)?;
        let memory = match self.instance.get_export(&mut self.store, MEMORY) {
            Some(Extern::Memory(memory)) => memory,
            _ => return Err("the guest does not export its memory".to_owned()),
        };
        memory
            .write(&mut self.store, pointer as usize, args)
            .map_err(|_| "the guest's argument buffer is outside its memory".to_owned())?;
        let packed = export
            .call(
                &mut self.store,
                (
                    parent.is_some() as i32,
                    parent.unwrap_or_default() as i64,
                    pointer,
                    args.len() as u32,
                ),
            )
            .map_err(failed)?;
        let (pointer, length) = direct::unpack(packed);
        let mut result = vec![0u8; length as usize];
        memory
            .read(&self.store, pointer as usize, &mut result)
            .map_err(|_| "the guest's result is outside its memory".to_owned())?;
        Ok(result)
    }

    pub fn take_outbound(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.store.data_mut().outbox)
    }

    pub fn on_wake(&self, notify: impl Fn() + Send + Sync + 'static) {
        self.wake.notify(notify);
    }

    pub fn take_wake(&self) -> bool {
        self.wake.take()
    }

    pub fn stop(&mut self) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        if !self.interrupted() {
            let _ = self.shutdown.call(&mut self.store, ());
        }
    }

    fn interrupted(&self) -> bool {
        self.store.data().interrupt.interrupted()
    }

    fn call(&mut self, function: TypedFunc<(), ()>) -> Result<(), String> {
        if self.stopped {
            return Ok(());
        }
        if self.interrupted() {
            self.stopped = true;
            return Err(INTERRUPTED.to_owned());
        }
        function.call(&mut self.store, ()).map_err(|error| {
            self.stopped = true;
            if self.store.data().interrupt.interrupted() {
                return INTERRUPTED.to_owned();
            }
            panic::message(&error).unwrap_or_else(|| format!("the guest trapped: {error:?}"))
        })
    }
}

impl Runnable for PureGuest {
    type Presented = Infallible;

    fn start(&mut self) -> Result<Produced<Infallible>, String> {
        PureGuest::start(self)?;
        Ok(Produced::sent(self.take_outbound(), StepTime::default()))
    }

    fn step(&mut self, frames: Vec<Vec<u8>>) -> Result<Produced<Infallible>, String> {
        self.take_wake();
        let started = Instant::now();
        self.deliver(frames)?;
        PureGuest::step(self)?;
        let took = StepTime {
            step: started.elapsed(),
            ..StepTime::default()
        };
        Ok(Produced::sent(self.take_outbound(), took))
    }

    fn take_wake(&mut self) -> bool {
        PureGuest::take_wake(self)
    }

    fn stop(&mut self) {
        PureGuest::stop(self);
    }
}

fn memory(caller: &mut Caller<'_, PureState>) -> wasmtime::Result<Memory> {
    match caller.get_export(MEMORY) {
        Some(Extern::Memory(memory)) => Ok(memory),
        _ => Err(wasmtime::Error::msg("the guest does not export its memory")),
    }
}

fn link(linker: &mut Linker<PureState>) -> Result<(), String> {
    linker
        .func_wrap(
            abi::HOST_MODULE,
            abi::HOST_SEND,
            |mut caller: Caller<'_, PureState>,
             pointer: u32,
             length: u32|
             -> wasmtime::Result<()> {
                let frame = read(&mut caller, pointer, length)?;
                caller.data_mut().outbox.push(frame);
                Ok(())
            },
        )
        .map_err(|error| format!("{} could not be linked: {error}", abi::HOST_SEND))?;
    linker
        .func_wrap(
            abi::HOST_MODULE,
            abi::HOST_RECEIVE,
            |mut caller: Caller<'_, PureState>,
             pointer: u32,
             capacity: u32|
             -> wasmtime::Result<i64> {
                let Some(needed) = caller.data().inbox.front().map(Vec::len) else {
                    return Ok(abi::NO_MESSAGE);
                };
                if needed > capacity as usize {
                    return Ok(needed as i64);
                }
                let frame = caller.data_mut().inbox.pop_front().unwrap_or_default();
                memory(&mut caller)?
                    .write(&mut caller, pointer as usize, &frame)
                    .map_err(|_| {
                        wasmtime::Error::msg("the guest's receive buffer is outside its memory")
                    })?;
                Ok(needed as i64)
            },
        )
        .map_err(|error| format!("{} could not be linked: {error}", abi::HOST_RECEIVE))?;
    linker
        .func_wrap(
            abi::HOST_MODULE,
            abi::HOST_PANIC,
            |mut caller: Caller<'_, PureState>,
             pointer: u32,
             length: u32|
             -> wasmtime::Result<()> {
                let message = match read(&mut caller, pointer, length) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
                    Err(error) => format!("the guest panicked, and its message was lost: {error}"),
                };
                Err(panic::panicked(message))
            },
        )
        .map_err(|error| format!("{} could not be linked: {error}", abi::HOST_PANIC))?;
    Ok(())
}

fn link_direct(linker: &mut Linker<PureState>, imports: &[(String, String)]) -> Result<(), String> {
    for (module, name) in imports {
        let full = format!("{module}#{name}");
        linker
            .func_wrap(
                module,
                name,
                move |mut caller: Caller<'_, PureState>,
                      has_parent: i32,
                      parent: i64,
                      pointer: u32,
                      length: u32|
                      -> wasmtime::Result<()> {
                    let args = read(&mut caller, pointer, length)?;
                    let parent = (has_parent != 0).then_some(parent as u64);
                    caller
                        .data_mut()
                        .outbox
                        .push(direct::call(&full, parent, false, &args));
                    Ok(())
                },
            )
            .map_err(|error| format!("{module}#{name} could not be linked: {error}"))?;
    }
    Ok(())
}

fn read(
    caller: &mut Caller<'_, PureState>,
    pointer: u32,
    length: u32,
) -> wasmtime::Result<Vec<u8>> {
    let memory = memory(caller)?;
    let mut bytes = vec![0u8; length as usize];
    memory
        .read(&*caller, pointer as usize, &mut bytes)
        .map_err(|_| wasmtime::Error::msg("a guest pointer ran past its memory"))?;
    Ok(bytes)
}
