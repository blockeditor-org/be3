use std::collections::BTreeSet;

use be_remap::names::{BUTTON_FIRST, LEFT_SUPER, Modifier, RIGHT_SUPER};
use be_remap::protocol::{decode, encode};
use be_remap::{Input, Output, Reply, Request, Source, Window};
use wasmi::{
    Config, Engine, Instance, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

pub use be_remap;

pub const START_FUEL: u64 = 50_000_000;
pub const CALL_FUEL: u64 = 5_000_000;
pub const MEMORY_BYTES: usize = 16 << 20;
pub const FAILURES_BEFORE_STOPPING: usize = 3;
pub const FAILURE_WINDOW: u64 = 60_000_000;
const SOONEST: u64 = 1000;
const MOST_REPLY_BYTES: usize = 1 << 20;
const MOST_OUTPUTS: usize = 1024;
const CODE_LIMIT: u16 = 0x300;
const BACKSPACE: u16 = 14;
const ESCAPE: u16 = 1;
const KEY_L: u16 = 38;
const TERMINAL_KEYS: [u16; 12] = [59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 87, 88];

pub struct Remapper {
    engine: Engine,
    module: Module,
    running: Option<Running>,
    config: Option<Vec<u8>>,
    window: Option<Window>,
    physical: BTreeSet<Source>,
    pressed: BTreeSet<Source>,
    bypassed: BTreeSet<Source>,
    locked: bool,
    failures: Vec<u64>,
    stopped: bool,
    deadline: Option<u64>,
    problems: Vec<String>,
}

struct Running {
    store: Store<StoreLimits>,
    memory: Memory,
    buffer: TypedFunc<u32, u32>,
    handle: TypedFunc<u32, u64>,
}

impl Remapper {
    pub fn new(module: &[u8]) -> Result<Self, String> {
        let mut config = Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, module)
            .map_err(|error| format!("this is not a remap module: {error}"))?;
        Ok(Self {
            engine,
            module,
            running: None,
            config: None,
            window: None,
            physical: BTreeSet::new(),
            pressed: BTreeSet::new(),
            bypassed: BTreeSet::new(),
            locked: false,
            failures: Vec::new(),
            stopped: false,
            deadline: None,
            problems: Vec::new(),
        })
    }

    pub fn configure(&mut self, config: Option<Vec<u8>>) -> Vec<Output> {
        if self.config == config {
            return Vec::new();
        }
        let out = self.release_held();
        self.config = config;
        self.stopped = false;
        self.failures.clear();
        out
    }

    pub fn focus(&mut self, window: Option<Window>, now: u64) -> Vec<Output> {
        if self.window == window {
            return Vec::new();
        }
        self.window = window.clone();
        if self.running.is_none() {
            return Vec::new();
        }
        match self.call(&Request::Focus(window), now) {
            Ok(outputs) => outputs,
            Err(error) => self.fail(&error, now),
        }
    }

    pub fn set_locked(&mut self, locked: bool) -> Vec<Output> {
        if self.locked == locked {
            return Vec::new();
        }
        self.locked = locked;
        match locked {
            true => self.release_held(),
            false => Vec::new(),
        }
    }

    pub fn input(&mut self, input: Input, now: u64) -> Vec<Output> {
        match input {
            Input::Press(source) => {
                self.physical.insert(source);
            }
            Input::Release(source) => {
                self.physical.remove(&source);
            }
            Input::Wheel { .. } => {}
        }
        if !self.active() {
            return self.deliver(vec![input]);
        }
        if let Input::Press(source) = input
            && self.reserved(source)
        {
            self.bypassed.insert(source);
            let mut outputs: Vec<Output> = Modifier::ALL
                .iter()
                .flat_map(|modifier| modifier.keys())
                .filter(|key| self.physical.contains(key))
                .map(Input::Press)
                .collect();
            outputs.push(input);
            return self.deliver(outputs);
        }
        if let Input::Release(source) = input
            && self.bypassed.remove(&source)
        {
            return self.deliver(vec![input]);
        }
        match self.call(&Request::Input { input, now }, now) {
            Ok(outputs) => outputs,
            Err(error) => {
                let mut out = self.fail(&error, now);
                out.extend(self.deliver(vec![input]));
                out
            }
        }
    }

    pub fn timer(&mut self, now: u64) -> Vec<Output> {
        if !self.active() || self.running.is_none() {
            self.deadline = None;
            return Vec::new();
        }
        match self.call(&Request::Timer { now }, now) {
            Ok(outputs) => outputs,
            Err(error) => self.fail(&error, now),
        }
    }

    pub fn deadline(&self) -> Option<u64> {
        self.deadline
    }

    pub fn running(&self) -> bool {
        self.active()
    }

    pub fn stopped(&self) -> bool {
        self.stopped
    }

    pub fn forget_held(&mut self) {
        self.running = None;
        self.deadline = None;
        self.physical.clear();
        self.pressed.clear();
        self.bypassed.clear();
    }

    pub fn take_problems(&mut self) -> Vec<String> {
        std::mem::take(&mut self.problems)
    }

    fn active(&self) -> bool {
        self.config.is_some() && !self.locked && !self.stopped
    }

    fn reserved(&self, source: Source) -> bool {
        let held = |modifier: Modifier| {
            modifier
                .keys()
                .iter()
                .any(|key| self.physical.contains(key))
        };
        let Source::Key(code) = source else {
            return false;
        };
        let held_super = self.physical.contains(&Source::Key(LEFT_SUPER))
            || self.physical.contains(&Source::Key(RIGHT_SUPER));
        (held(Modifier::Ctrl)
            && held(Modifier::Alt)
            && (code == BACKSPACE || TERMINAL_KEYS.contains(&code)))
            || (held_super && (code == KEY_L || code == ESCAPE))
    }

    fn call(&mut self, request: &Request, now: u64) -> Result<Vec<Output>, String> {
        self.start(now)?;
        let Some(running) = self.running.as_mut() else {
            return Err("the remaps did not start".to_owned());
        };
        let reply = running.call(&encode(request), CALL_FUEL)?;
        Ok(self.settle(reply, now))
    }

    fn start(&mut self, now: u64) -> Result<(), String> {
        if self.running.is_some() {
            return Ok(());
        }
        let config = self.config.clone().unwrap_or_default();
        let mut running = Running::new(&self.engine, &self.module)?;
        let request = Request::Start {
            config,
            window: self.window.clone(),
            held: self.physical.iter().copied().collect(),
            pressed: self.pressed.iter().copied().collect(),
        };
        let reply = running.call(&encode(&request), START_FUEL)?;
        self.running = Some(running);
        let outputs = self.settle(reply, now);
        if outputs.is_empty() {
            return Ok(());
        }
        Err("the remaps pressed keys while starting".to_owned())
    }

    fn settle(&mut self, reply: Reply, now: u64) -> Vec<Output> {
        self.deadline = reply
            .deadline
            .map(|deadline| deadline.max(now.saturating_add(SOONEST)));
        let outputs = reply
            .outputs
            .into_iter()
            .take(MOST_OUTPUTS)
            .filter(|output| match output {
                Input::Press(source) | Input::Release(source) => valid(*source),
                Input::Wheel { .. } => true,
            })
            .collect();
        self.deliver(outputs)
    }

    fn deliver(&mut self, outputs: Vec<Output>) -> Vec<Output> {
        outputs
            .into_iter()
            .filter(|output| match output {
                Input::Press(source) => self.pressed.insert(*source),
                Input::Release(source) => self.pressed.remove(source),
                Input::Wheel { .. } => true,
            })
            .collect()
    }

    fn fail(&mut self, error: &str, now: u64) -> Vec<Output> {
        let out = self.release_held();
        self.failures
            .retain(|at| now.saturating_sub(*at) < FAILURE_WINDOW);
        self.failures.push(now);
        if self.failures.len() >= FAILURES_BEFORE_STOPPING {
            self.stopped = true;
            self.problems.push(format!(
                "The input remaps stopped after failing {FAILURES_BEFORE_STOPPING} times in a minute ({error}). Keys and buttons reach the windows unchanged until the remaps change."
            ));
        } else {
            self.problems.push(format!(
                "The input remaps failed ({error}) and start again with the next key or button."
            ));
        }
        out
    }

    fn release_held(&mut self) -> Vec<Output> {
        self.running = None;
        self.deadline = None;
        let released: Vec<Source> = self
            .pressed
            .iter()
            .copied()
            .filter(|source| !self.physical.contains(source))
            .collect();
        released
            .into_iter()
            .filter(|source| self.pressed.remove(source))
            .map(Input::Release)
            .collect()
    }
}

fn valid(source: Source) -> bool {
    match source {
        Source::Key(code) => code > 0 && code < BUTTON_FIRST,
        Source::Button(code) => (BUTTON_FIRST..CODE_LIMIT).contains(&code),
    }
}

impl Running {
    fn new(engine: &Engine, module: &Module) -> Result<Self, String> {
        let limits = StoreLimitsBuilder::new().memory_size(MEMORY_BYTES).build();
        let mut store = Store::new(engine, limits);
        store.limiter(|limits| limits);
        store
            .set_fuel(START_FUEL)
            .map_err(|error| format!("the remaps could not be given fuel: {error}"))?;
        let linker = Linker::<StoreLimits>::new(engine);
        let instance = linker
            .instantiate_and_start(&mut store, module)
            .map_err(|error| format!("the remaps would not start: {error}"))?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| "the remap module exports no memory".to_owned())?;
        Ok(Self {
            buffer: export(&store, &instance, "be_remap_buffer")?,
            handle: export(&store, &instance, "be_remap_handle")?,
            store,
            memory,
        })
    }

    fn call(&mut self, request: &[u8], fuel: u64) -> Result<Reply, String> {
        let length = u32::try_from(request.len())
            .map_err(|_| "the request is too long for the remaps".to_owned())?;
        self.store
            .set_fuel(fuel)
            .map_err(|error| format!("the remaps could not be given fuel: {error}"))?;
        let pointer = self
            .buffer
            .call(&mut self.store, length)
            .map_err(|error| stopped(&error))?;
        self.memory
            .write(&mut self.store, pointer as usize, request)
            .map_err(|_| "the remaps gave a buffer outside their memory".to_owned())?;
        let answer = self
            .handle
            .call(&mut self.store, length)
            .map_err(|error| stopped(&error))?;
        let pointer = (answer >> 32) as usize;
        let length = (answer & u64::from(u32::MAX)) as usize;
        if length > MOST_REPLY_BYTES {
            return Err("the remaps answered at too great a length".to_owned());
        }
        let end = pointer
            .checked_add(length)
            .ok_or_else(|| "the remaps answered outside their memory".to_owned())?;
        let reply = self
            .memory
            .data(&self.store)
            .get(pointer..end)
            .ok_or_else(|| "the remaps answered outside their memory".to_owned())?;
        decode(reply).map_err(|error| format!("the remaps answered with nothing readable: {error}"))
    }
}

fn stopped(error: &wasmi::Error) -> String {
    match error.as_trap_code() {
        Some(wasmi::TrapCode::OutOfFuel) => "they ran out of fuel".to_owned(),
        _ => format!("they stopped: {error}"),
    }
}

fn export<Parameters: wasmi::WasmParams, Results: wasmi::WasmResults>(
    store: &Store<StoreLimits>,
    instance: &Instance,
    name: &str,
) -> Result<TypedFunc<Parameters, Results>, String> {
    instance
        .get_typed_func(store, name)
        .map_err(|error| format!("the remap module has no usable {name}: {error}"))
}

#[cfg(test)]
mod tests;
