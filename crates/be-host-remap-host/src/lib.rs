use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use be_lib_failure_budget::{Failed, FailureBudget};
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
pub const FAILURE_WINDOW: Duration = Duration::from_secs(60);
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
    /// The keys in `bypassed` that were pressed while the remaps started, rather than with a
    /// session chord: once the remaps answer `Start`, they are told of them and their releases go
    /// to the remaps. Key repeats never reach the filter (libinput sends presses and releases
    /// only), so a held key is pressed once.
    bypassed_starting: BTreeSet<Source>,
    forced: BTreeSet<Source>,
    locked: bool,
    failures: FailureBudget,
    stopped: bool,
    deadline: Option<u64>,
    problems: Vec<String>,
}

/// The remaps' instance, which runs in the sandbox on a thread of its own and answers each
/// request with one frame.
struct Running {
    instance: Instance,
    /// While `Start` is unanswered, the time on the input clock it must be answered by, which
    /// `deadline` reports so the host's timer comes then. Input passes through unchanged
    /// meanwhile, so it never waits for a start.
    starting: Option<u64>,
    /// What `Start` told them.
    told: Told,
}

/// The input and window the remaps were started with.
#[derive(Clone, PartialEq, Default)]
struct Told {
    window: Option<Window>,
    held: BTreeSet<Source>,
    pressed: BTreeSet<Source>,
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
            bypassed_starting: BTreeSet::new(),
            forced: BTreeSet::new(),
            locked: false,
            failures: FailureBudget::new(FAILURES_BEFORE_STOPPING, FAILURE_WINDOW),
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
        self.failures.reset();
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
        // Taking a start's answer first, so a fresh `Start` is not told of this input too.
        let mut out = self.poll(now);
        out.extend(self.remap_polled(input, now));
        out
    }

    fn remap_polled(&mut self, input: Input, now: u64) -> Vec<Output> {
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
        let mut out = Vec::new();
        // Until the remaps answer `Start` (or after it failed), input passes through as it is,
        // and so does the release of a key pressed meanwhile.
        if !self.ready() {
            if let Input::Press(source) = input
                && self.active()
                && self.bypassed.insert(source)
            {
                self.bypassed_starting.insert(source);
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
    /// it, and input and the timer call it too. A start not answered by `Limits::start` on the
    /// input clock fails when the timer `deadline` asks for comes.
    pub fn poll(&mut self, now: u64) -> Vec<Output> {
        if !self.active() {
            return Vec::new();
        }
        let Some(running) = self.running.as_mut() else {
            return match self.start(now) {
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
            [] if now >= by => return self.fail("they did not answer in time", now),
            [] => return Vec::new(),
            [reply] => read(reply),
            _ => Err("they answered more than once".to_owned()),
        };
        match reply.and_then(|reply| self.started(reply, now)) {
            Ok(out) => out,
            Err(error) => self.fail(&error, now),
        }
    }

    /// Takes the remaps' answer to `Start`. Should the keys held or pressed, or the window, have
    /// changed since it was sent, they are started again with what is true now, waiting as for any
    /// call, so they never act on input they missed.
    fn started(&mut self, reply: Reply, now: u64) -> Result<Vec<Output>, String> {
        if !self.take_start(&reply)? {
            return Ok(Vec::new());
        }
        // The keys pressed while they started are theirs from now: told below as held, should they
        // still be, and their releases going to them.
        for source in std::mem::take(&mut self.bypassed_starting) {
            self.bypassed.remove(&source);
        }
        let told = self.told();
        let Some(running) = self.running.as_mut() else {
            return Err("the remaps did not start".to_owned());
        };
        let reply = match running.told == told {
            true => reply,
            false => {
                let reply = running.call(&start_request(&self.config, &told), self.limits.call)?;
                if !self.take_start(&reply)? {
                    return Ok(Vec::new());
                }
                reply
            }
        };
        if let Some(running) = self.running.as_mut() {
            running.starting = None;
            running.told = told;
        }
        Ok(self.settle(reply, now))
    }

    /// Whether the remaps may run after this answer to `Start`: not if their config could not be
    /// read, which stops them. Remaps that press keys as they start fail.
    fn take_start(&mut self, reply: &Reply) -> Result<bool, String> {
        if let Some(problem) = &reply.problem {
            self.running = None;
            self.stopped = true;
            self.problems.push(format!(
                "The input remaps could not be read, so they stay off until they change: {problem}"
            ));
            return Ok(false);
        }
        match reply.outputs.is_empty() {
            true => Ok(true),
            false => Err("the remaps pressed keys while starting".to_owned()),
        }
    }

    /// What the remaps are told as they start: everything but the keys that bypass them, whose
    /// releases pass as they are.
    fn told(&self) -> Told {
        Told {
            window: self.window.clone(),
            held: self.physical.difference(&self.bypassed).copied().collect(),
            pressed: self.pressed.difference(&self.bypassed).copied().collect(),
        }
    }

    pub fn timer(&mut self, now: u64) -> Vec<Output> {
        if !self.ready() {
            return self.poll(now);
        }
        match self.call(&Request::Timer { now }, now) {
            Ok(outputs) => outputs,
            Err(error) => self.fail(&error, now),
        }
    }

    /// When the host's timer should call `timer`: when the remaps asked, or when their start
    /// must have been answered by.
    pub fn deadline(&self) -> Option<u64> {
        match self.running.as_ref().and_then(|running| running.starting) {
            Some(by) => Some(by),
            None => self.deadline,
        }
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
        self.bypassed_starting.clear();
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

    /// Whether the remaps should run and are still starting (or about to).
    #[cfg(test)]
    fn starting(&self) -> bool {
        self.active() && !self.ready()
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
    fn start(&mut self, now: u64) -> Result<(), String> {
        let mut running = Running::new(&self.module, Arc::clone(&self.wake))?;
        let told = self.told();
        running
            .instance
            .send(vec![start_request(&self.config, &told)])
            .map_err(|error| format!("they stopped: {error}"))?;
        running.starting = Some(now.saturating_add(micros(self.limits.start)));
        running.told = told;
        self.running = Some(running);
        self.deadline = None;
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
        if self.failures.fail(Duration::from_micros(now)) == Failed::Stop {
            self.stopped = true;
            self.problems.push(format!(
                "The input remaps stopped after failing {FAILURES_BEFORE_STOPPING} times in a minute ({error}). Keys and buttons reach the windows unchanged until the remaps change."
            ));
        } else {
            self.problems.push(format!(
                "The input remaps failed ({error}) and start again."
            ));
        }
        out
    }

    fn release_held(&mut self) -> Vec<Output> {
        self.running = None;
        self.deadline = None;
        // Keys pressed while the remaps started stay bypassed: their releases pass as they are.
        self.bypassed_starting.clear();
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
            display: None,
        };
        let instance = sandbox::spawn(
            // A remap draws on no randomness that should differ between runs.
            sandbox::Source::Module {
                module: module.clone(),
                seed: [0; 32],
            },
            config,
            wake,
        )
            .map_err(|error| format!("the remaps would not start: {error}"))?;
        Ok(Self {
            instance,
            starting: None,
            told: Told::default(),
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
        read(reply)
    }
}

fn read(reply: &[u8]) -> Result<Reply, String> {
    if reply.len() > MOST_REPLY_BYTES {
        return Err("the remaps answered at too great a length".to_owned());
    }
    decode(reply).map_err(|error| format!("the remaps answered with nothing readable: {error}"))
}

fn start_request(config: &Option<Vec<u8>>, told: &Told) -> Vec<u8> {
    encode(&Request::Start {
        config: config.clone().unwrap_or_default(),
        window: told.window.clone(),
        held: told.held.iter().copied().collect(),
        pressed: told.pressed.iter().copied().collect(),
    })
}

/// `duration` on the input clock, which counts microseconds.
fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests;
