//! Workers: work an app runs off its own thread, as an instance of a module of its own that it
//! talks to by messages (plans/single_threaded_guests.md, Workers; guides/adding_an_app.md,
//! Workers). be-sdk re-exports it as `be_sdk::worker`.
//!
//! - The app names each worker module in its manifest's `workers`, and its BUCK file builds and
//!   stages it (`app(workers = ...)`).
//! - The worker crate implements `Work`, and its `worker.rs` exports it with
//!   `be_sdk_worker::worker!`, the module's entry point. It runs in an instance of its own: on a
//!   thread and isolate of its own natively, in a worker of its own in the browser, so it may take
//!   as long as a job needs without holding up the app's frames. It has WASI and its libraries,
//!   but no gpu: it computes, and sends what it made back as bytes.
//! - The app starts it with `Worker::spawn` through the sandbox API (`be3:sandbox`), sends it
//!   messages with `send` and takes its answers with `receive`. Each message is one byte buffer,
//!   delivered whole and in order. Dropping the `Worker` stops it.
//! - Or, for work every editor of the app asks of one worker, each holds a `Shared`: they share
//!   one instance of the module, and each hears only the answers to what it asked.
//!
//! Each message travels with the client that sent it or that it answers, which `Shared` tells its
//! editors apart by: a `Work` answers the client whose message it is taking, and `Worker` is
//! client 0, as are the messages a worker sends as it starts.
//!
//! Nothing here assumes the guest has one thread: it keeps no state of its own, and a `Worker` is
//! used from the thread whose executor spawned it, as every op is.

use std::{collections::VecDeque, rc::Rc};

mod shared;

pub use shared::Shared;

use be_wasm_sandbox_api::{
    guest::sandbox::Instance,
    ops::OpError,
    sandbox::{Config, Source, StagedModule},
};

#[doc(hidden)]
pub mod __private {
    pub use be_wasm_guest_abi::{Flow, Guest, Waker, guest};
}

/// The most bytes a worker's memory may hold, as for an app; the host holds it to that too.
pub const MEMORY_LIMIT: u64 = 1 << 30;

/// Why a worker could not start on a host that serves the app no sandbox, such as a wire a test
/// made by itself.
pub const NO_WORKERS: &str = "this host runs no workers";

/// A worker the app started: an instance of one of its worker modules.
pub struct Worker {
    sender: Sender,
    received: VecDeque<Vec<u8>>,
}

impl Worker {
    /// Starts `module`, one of the worker modules the app's manifest names under `workers`
    /// (such as `"tiles.wasm"`), as an instance of its own. One that cannot start, such as a
    /// module the app does not stage, says why from `receive`. Drop a `Worker` (and its
    /// `Sender`s) once `receive` says it stopped: a stopped worker stays held until then.
    pub fn spawn(module: &str) -> Self {
        // The host serves an app the sandbox for its own workers, so no app id names them.
        let source = Source::App(StagedModule {
            app: String::new(),
            module: module.to_owned(),
        });
        let config = Config {
            name: format!("worker {module}"),
            memory_limit: MEMORY_LIMIT,
        };
        Self {
            sender: Sender(Rc::new(Instance::new(source, config))),
            received: VecDeque::new(),
        }
    }

    /// Sends the worker one message. Messages arrive in the order they were sent; one sent to a
    /// worker that stopped is dropped, and `receive` says why it stopped.
    pub fn send(&self, message: Vec<u8>) {
        self.sender.send(message);
    }

    /// Something that sends to this worker, for a task that sends while another receives.
    pub fn sender(&self) -> Sender {
        self.sender.clone()
    }

    /// The worker's next message, once it has sent one, or why it stopped: it could not start,
    /// trapped or panicked. Once it stopped, every later call fails the same way.
    pub async fn receive(&mut self) -> Result<Vec<u8>, String> {
        self.receive_framed().await.map(|(_, message)| message)
    }

    /// The worker's next message and the client it answers (`Shared`).
    async fn receive_framed(&mut self) -> Result<(u64, Vec<u8>), String> {
        loop {
            if let Some(frame) = self.received.pop_front() {
                return unframe(&frame).map(|(client, message)| (client, message.to_vec()));
            }
            let frames = self.sender.0.next().await.map_err(|error| match error {
                OpError::Unsupported => NO_WORKERS.to_owned(),
                error => format!("the host did not answer: {error:?}"),
            })??;
            self.received.extend(frames);
        }
    }
}

/// Sends messages to a worker (`Worker::sender`). The worker stops once its `Worker` and every
/// `Sender` are dropped.
#[derive(Clone)]
pub struct Sender(Rc<Instance>);

impl Sender {
    /// Sends the worker one message (`Worker::send`).
    pub fn send(&self, message: Vec<u8>) {
        self.send_as(0, &message);
    }

    fn send_as(&self, client: u64, message: &[u8]) {
        self.0.send(vec![frame(client, message)]).detach();
    }
}

/// A message as it travels between an app and its worker: the client it is from or for, then the
/// message.
fn frame(client: u64, message: &[u8]) -> Vec<u8> {
    [&client.to_le_bytes()[..], message].concat()
}

fn unframe(frame: &[u8]) -> Result<(u64, &[u8]), String> {
    let Some((client, message)) = frame.split_first_chunk::<8>() else {
        return Err("a worker's message came with no client".to_owned());
    };
    Ok((u64::from_le_bytes(*client), message))
}

/// What a worker module does: its state, made when it starts, and how it takes each message from
/// the app that spawned it. Export it with `worker!`.
pub trait Work: Sized + 'static {
    /// Starts the worker.
    fn start(app: &mut App) -> Result<Self, String>;

    /// Takes one message from the app. An error stops the worker, and the app's `receive` says
    /// why.
    fn receive(&mut self, message: Vec<u8>, app: &mut App) -> Result<(), String>;
}

/// The app that spawned a worker, as the worker sees it.
pub struct App<'a> {
    outbound: &'a mut Vec<Vec<u8>>,
    /// The client whose message the worker is taking, which what it sends answers.
    client: u64,
}

impl App<'_> {
    /// Sends the app one message, which its `Worker::receive` answers with: an answer to the
    /// client whose message the worker is taking (`Shared`), or to client 0 as it starts.
    pub fn send(&mut self, message: Vec<u8>) {
        self.outbound.push(frame(self.client, &message));
    }
}

/// Starts `W`, which sends what it says as it starts to `outbound`.
fn start<W: Work>(outbound: &mut Vec<Vec<u8>>) -> Result<W, String> {
    W::start(&mut App {
        outbound,
        client: 0,
    })
}

/// Hands `W` each of `frames`, as its app sent them, answering to `outbound`.
fn step<W: Work>(
    work: &mut W,
    frames: Vec<Vec<u8>>,
    outbound: &mut Vec<Vec<u8>>,
) -> Result<(), String> {
    for frame in frames {
        let (client, message) = unframe(&frame)?;
        work.receive(message.to_vec(), &mut App { outbound, client })?;
    }
    Ok(())
}

/// A `Work` run in the caller's own instance rather than one of its own: how an app's tests
/// serve its workers (be-sdk-test's `BeuiTest::serve_worker`), and a worker crate's tests drive
/// it, in step with the test. Frames in and out are as an instance of the worker module takes and
/// sends them.
pub struct Hosted(Box<dyn FnMut(Vec<Vec<u8>>, &mut Vec<Vec<u8>>) -> Result<(), String>>);

impl Hosted {
    /// Starts `W`, which sends what it says as it starts to `outbound`.
    pub fn start<W: Work>(outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        let mut work = start::<W>(outbound)?;
        Ok(Self(Box::new(move |frames, outbound| {
            step(&mut work, frames, outbound)
        })))
    }

    /// Hands the worker the frames its app sent, which it answers to `outbound`. An error stops
    /// it, as it stops an instance.
    pub fn step(
        &mut self,
        frames: Vec<Vec<u8>>,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<(), String> {
        (self.0)(frames, outbound)
    }

    /// Sends the worker one message as a `Worker` would, and answers what it sent back: how a
    /// worker crate's tests ask it something.
    pub fn ask(&mut self, message: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        let mut outbound = Vec::new();
        self.step(vec![frame(0, message)], &mut outbound)?;
        outbound
            .iter()
            .map(|frame| unframe(frame).map(|(_, message)| message.to_vec()))
            .collect()
    }
}

/// Runs a `Work` as a guest: what `worker!` exports.
#[doc(hidden)]
pub struct Running<W>(W);

impl<W: Work> be_wasm_guest_abi::Guest for Running<W> {
    fn start(
        _waker: be_wasm_guest_abi::Waker,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Self, String> {
        start::<W>(outbound).map(Self)
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _woken: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<be_wasm_guest_abi::Flow, String> {
        step(&mut self.0, inbound, outbound)?;
        Ok(be_wasm_guest_abi::Flow::Running)
    }
}

/// The entry point of a worker module: exports `$work`, a `Work`, as the module's `app_start`.
/// A worker crate calls it in its `worker.rs` alone, which buck links into the module
/// (`worker()`), so the app and tests that depend on the crate link no second entry point
/// (guides/adding_an_app.md, Workers).
#[macro_export]
macro_rules! worker {
    ($work:ty) => {
        $crate::__private::guest!($crate::Running<$work>);
    };
}
