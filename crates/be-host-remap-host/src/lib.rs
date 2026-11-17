use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use be_lib_remap::names::{BUTTON_FIRST, CODE_LIMIT, Modifier};
use be_lib_remap::protocol::{decode, encode};
use be_lib_remap::{Input, Output, Reply, Request, Source, Window};
use be_wasm_guest_host::Deadline;
use be_wasm_host::sandbox::{self, Instance};

pub use be_lib_remap;
pub use be_wasm_guest_host::Wake;
pub use be_wasm_host::sandbox::{Compiling, Module};

pub mod capture;

/// How long the remaps may take to answer, past which they fail and are terminated.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// From spawning their instance to answering `Start`, which reads their config: generous
    /// enough for a loaded machine or a slow device.
    pub start: Duration,
    /// For anything else. Input waits for them meanwhile.
    pub call: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            start: Duration::from_secs(5),
            call: Duration::from_millis(100),
        }
    }
}
pub const MEMORY_BYTES: usize = 16 << 20;
pub const FAILURES_BEFORE_STOPPING: usize = 3;
pub const FAILURE_WINDOW: u64 = 60_000_000;
const SOONEST: u64 = 1000;
const MOST_REPLY_BYTES: usize = 1 << 20;
const MOST_OUTPUTS: usize = 1024;
const BACKSPACE: u16 = 14;
const ESCAPE: u16 = 1;
const KEY_L: u16 = 38;
const TERMINAL_KEYS: [u16; 12] = [59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 87, 88];

pub struct Remapper {
    module: Module,
    wake: Wake,
    limits: Limits,
    running: Option<Running>,
    config: Option<Vec<u8>>,
    window: Option<Window>,
    physical: BTreeSet<Source>,
    pressed: BTreeSet<Source>,
    bypassed: BTreeSet<Source>,
    forced: BTreeSet<Source>,
    locked: bool,
    failures: Vec<u64>,
    stopped: bool,
    deadline: Option<u64>,
    problems: Vec<String>,
}

/// The remaps' instance, which runs in the sandbox on a thread of its own and answers each
/// request with one frame.
struct Running {
    instance: Instance,
    /// While `Start` is unanswered, when it must be answered by. Input passes through unchanged
    /// meanwhile, so it never waits for a start.
    starting: Option<Deadline>,
    /// The window `Start` named.
    window: Option<Window>,
}

impl Remapper {
    /// Compiles a remap module in the sandbox, off the caller's thread, and calls `wake` once
    /// it is done; `Remapper::new` takes what it compiled.
    pub fn compile(module: Vec<u8>, wake: Wake) -> Compiling {
        sandbox::compile(module, wake)
    }

    /// Remaps running `module`, whose instance calls `wake` when it answers, after which the
    /// host calls `poll`.
    pub fn new(module: Module, wake: Wake) -> Self {
        Self {
            module,
            wake,
            limits: Limits::default(),
            running: None,
            config: None,
            window: None,
            physical: BTreeSet::new(),
            pressed: BTreeSet::new(),
            bypassed: BTreeSet::new(),
            forced: BTreeSet::new(),
            locked: false,
            failures: Vec::new(),
            stopped: false,
            deadline: None,
            problems: Vec::new(),
        }
    }

    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
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
        // Remaps still starting are told once `Start` is answered (`poll`).
        if !self.ready() {
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
        let mut out = self.remap(input, now);
        // A modifier the session's own keys pressed past the remaps is let go with its own
        // release, whatever the remaps make of that release.
        if let Input::Release(source) = input
            && self.forced.remove(&source)
        {
            out.extend(self.deliver(vec![input]));
        }
        out
    }

    fn remap(&mut self, input: Input, now: u64) -> Vec<Output> {
        match input {
            Input::Press(source) => {
                self.physical.insert(source);
            }
            // A key pressed before these remaps saw it (before they were installed, or before the
            // session left its terminal) went on as it was, so its release does too.
            Input::Release(source) if !self.physical.remove(&source) => return vec![input],
            Input::Release(_) | Input::Wheel { .. } => {}
        }
        if !self.active() {
            return self.deliver(vec![input]);
        }
        if let Input::Press(source) = input
            && reserved(&self.physical, source)
        {
            self.bypassed.insert(source);
            let mut outputs: Vec<Output> = Modifier::ALL
                .iter()
                .flat_map(|modifier| modifier.keys())
                .filter(|key| self.physical.contains(key))
                .map(Input::Press)
                .collect();
            for output in &outputs {
                if let Input::Press(modifier) = output
                    && !self.pressed.contains(modifier)
                {
                    self.forced.insert(*modifier);
                }
            }
            outputs.push(input);
            return self.deliver(outputs);
        }
        if let Input::Release(source) = input
            && self.bypassed.remove(&source)
        {
            return self.deliver(vec![input]);
        }
        let mut out = self.poll(now);
        // Until the remaps answer `Start` (or after it failed), input passes through as it is,
        // and so does the release of a key pressed meanwhile.
        if !self.ready() {
            if let Input::Press(source) = input
                && self.active()
            {
                self.bypassed.insert(source);
            }
            out.extend(self.deliver(vec![input]));
            return out;
        }
        match self.call(&Request::Input { input, now }, now) {
            Ok(outputs) => out.extend(outputs),
            Err(error) => {
                out.extend(self.fail(&error, now));
                out.extend(self.deliver(vec![input]));
            }
        }
        out
    }

    /// Starts the remaps when they should run and are not running, and takes their answer to
    /// `Start` once it came, without waiting: the host calls this whenever their instance wakes
    /// it, and input calls it too. A start not answered within `Limits::start` fails.
    pub fn poll(&mut self, now: u64) -> Vec<Output> {
        if !self.active() {
            return Vec::new();
        }
        let Some(running) = self.running.as_mut() else {
            return match self.start() {
                Ok(()) => Vec::new(),
                Err(error) => self.fail(&error, now),
            };
        };
        let Some(by) = running.starting else {
            return Vec::new();
        };
        let answers = match running.instance.receive() {
            Ok(answers) => answers,
            Err(error) => return self.fail(&format!("they stopped: {error}"), now),
        };
        let reply = match answers.as_slice() {
            [] if by.remaining().is_none() => {
                return self.fail("they did not answer in time", now);
            }
            [] => return Vec::new(),
            [reply] if reply.len() > MOST_REPLY_BYTES => {
                return self.fail("the remaps answered at too great a length", now);
            }
            [reply] => decode::<Reply>(reply),
            _ => return self.fail("they answered more than once", now),
        };
        let reply = match reply {
            Ok(reply) => reply,
            Err(error) => {
                return self.fail(
                    &format!("the remaps answered with nothing readable: {error}"),
                    now,
                );
            }
        };
        if let Some(problem) = reply.problem {
            self.running = None;
            self.stopped = true;
            self.problems.push(format!(
                "The input remaps could not be read, so they stay off until they change: {problem}"
            ));
            return Vec::new();
        }
        if !reply.outputs.is_empty() {
            return self.fail("the remaps pressed keys while starting", now);
        }
        running.starting = None;
        let started_in = running.window.clone();
        let mut out = self.settle(reply, now);
        if started_in != self.window {
            match self.call(&Request::Focus(self.window.clone()), now) {
                Ok(outputs) => out.extend(outputs),
                Err(error) => out.extend(self.fail(&error, now)),
            }
        }
        out
    }

    /// Whether the remaps should run and are still starting (or about to).
    pub fn starting(&self) -> bool {
        self.active() && !self.ready()
    }

    pub fn timer(&mut self, now: u64) -> Vec<Output> {
        if !self.ready() {
            self.deadline = None;
            return self.poll(now);
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
        self.forced.clear();
    }

    pub fn take_problems(&mut self) -> Vec<String> {
        std::mem::take(&mut self.problems)
    }

    fn active(&self) -> bool {
        self.config.is_some() && !self.locked && !self.stopped
    }

    fn ready(&self) -> bool {
        self.active()
            && self
                .running
                .as_ref()
                .is_some_and(|running| running.starting.is_none())
    }

    fn call(&mut self, request: &Request, now: u64) -> Result<Vec<Output>, String> {
        let Some(running) = self.running.as_mut() else {
            return Err("the remaps did not start".to_owned());
        };
        let reply = running.call(&encode(request), self.limits.call)?;
        Ok(self.settle(reply, now))
    }

    /// Spawns the remaps' instance and sends it `Start`, which `poll` takes the answer to.
    fn start(&mut self) -> Result<(), String> {
        let mut running = Running::new(&self.module, Arc::clone(&self.wake))?;
        let request = Request::Start {
            config: self.config.clone().unwrap_or_default(),
            window: self.window.clone(),
            held: self.physical.iter().copied().collect(),
            pressed: self.pressed.iter().copied().collect(),
        };
        running
            .instance
            .send(vec![encode(&request)])
            .map_err(|error| format!("they stopped: {error}"))?;
        running.starting = Some(Deadline::after(self.limits.start));
        running.window = self.window.clone();
        self.running = Some(running);
        Ok(())
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

/// The session's own keys, which neither a remapper nor a capture may keep from it:
/// Ctrl+Alt+Backspace, Ctrl+Alt+F<n>, Super+L and Super+Escape.
fn reserved(physical: &BTreeSet<Source>, source: Source) -> bool {
    let held = |modifier: Modifier| modifier.keys().iter().any(|key| physical.contains(key));
    let Source::Key(code) = source else {
        return false;
    };
    (held(Modifier::Ctrl)
        && held(Modifier::Alt)
        && (code == BACKSPACE || TERMINAL_KEYS.contains(&code)))
        || (held(Modifier::Super) && (code == KEY_L || code == ESCAPE))
}

fn valid(source: Source) -> bool {
    match source {
        Source::Key(code) => code > 0 && code < BUTTON_FIRST,
        Source::Button(code) => (BUTTON_FIRST..CODE_LIMIT).contains(&code),
    }
}

impl Running {
    fn new(module: &Module, wake: Wake) -> Result<Self, String> {
        let config = sandbox::Config {
            name: "input remaps".to_owned(),
            memory_limit: MEMORY_BYTES,
        };
        let instance = sandbox::spawn(module, config, wake)
            .map_err(|error| format!("the remaps would not start: {error}"))?;
        Ok(Self {
            instance,
            starting: None,
            window: None,
        })
    }

    /// Sends `request` and waits up to `within` for the one frame that answers it. A module that
    /// takes longer is dropped by the caller, which terminates it.
    fn call(&mut self, request: &[u8], within: Duration) -> Result<Reply, String> {
        let deadline = Deadline::after(within);
        self.instance
            .send(vec![request.to_vec()])
            .map_err(|error| format!("they stopped: {error}"))?;
        let answers = self
            .instance
            .wait(deadline)
            .map_err(|error| format!("they stopped: {error}"))?;
        let [reply] = answers.as_slice() else {
            return Err(match answers.is_empty() {
                true => "they did not answer in time".to_owned(),
                false => "they answered more than once".to_owned(),
            });
        };
        if reply.len() > MOST_REPLY_BYTES {
            return Err("the remaps answered at too great a length".to_owned());
        }
        decode(reply).map_err(|error| format!("the remaps answered with nothing readable: {error}"))
    }
}

#[cfg(test)]
mod tests;
