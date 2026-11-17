//! Workers: work an app runs off its own thread, as an instance of a module of its own that it
//! talks to by messages (plans/single_threaded_guests.md, Workers; guides/adding_an_app.md,
//! Workers). be-sdk re-exports it as `be_sdk::worker`.
//!
//! - The app names each worker module in its manifest's `workers`, and its BUCK file builds and
//!   stages it (`app(workers = ...)`).
//! - The worker module implements `Work` and exports it with `be_sdk_worker::worker!`, its entry
//!   point. Natively it runs on a thread and isolate of its own, so it may take as long as a job
//!   needs without holding up the app's frames. It has WASI and its libraries, but no gpu: it
//!   computes, and sends what it made back as bytes.
//! - The app starts it with `Worker::spawn` through the sandbox API (`be3:sandbox`), sends it
//!   messages with `send` and takes its answers with `receive`. Each message is one byte buffer,
//!   delivered whole and in order. Dropping the `Worker` stops it.
//!
//! Nothing here assumes the guest has one thread: it keeps no state of its own, and a `Worker` is
//! used from the thread whose executor spawned it, as every op is.

use std::{collections::VecDeque, rc::Rc};

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

/// Why a worker could not start on a host that serves no sandbox, such as the web's today.
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
        loop {
            if let Some(message) = self.received.pop_front() {
                return Ok(message);
            }
            let messages = self.sender.0.next().await.map_err(|error| match error {
                OpError::Unsupported => NO_WORKERS.to_owned(),
                error => format!("the host did not answer: {error:?}"),
            })??;
            self.received.extend(messages);
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
        self.0.send(vec![message]).detach();
    }
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
}

impl App<'_> {
    /// Sends the app one message, which its `Worker::receive` answers with.
    pub fn send(&mut self, message: Vec<u8>) {
        self.outbound.push(message);
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
        W::start(&mut App { outbound }).map(Self)
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _woken: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<be_wasm_guest_abi::Flow, String> {
        let mut app = App { outbound };
        for message in inbound {
            self.0.receive(message, &mut app)?;
        }
        Ok(be_wasm_guest_abi::Flow::Running)
    }
}

/// The entry point of a worker module: exports `$work`, a `Work`, as the module's `app_start`.
/// A worker module is a crate of its own, built as an app's module is
/// (guides/adding_an_app.md, Workers).
#[macro_export]
macro_rules! worker {
    ($work:ty) => {
        $crate::__private::guest!($crate::Running<$work>);
    };
}
