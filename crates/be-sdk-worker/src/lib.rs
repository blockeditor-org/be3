//! Workers: work an app runs off its own thread, as an instance of a module of its own that it
//! talks to by messages, since a guest is single-threaded (decisions/guests_are_single_threaded.md;
//! guides/adding_an_app.md, Workers). be-sdk re-exports it as `be_sdk::worker`.
//!
//! - The app names each worker module in its manifest's `workers`, and its BUCK file builds and
//!   stages it (`app(workers = ...)`).
//! - The worker crate implements `Work`, and its `worker.rs` exports it with
//!   `be_sdk_worker::worker!`, the module's entry point. It runs in an instance of its own: on a
//!   thread and isolate of its own natively, in a worker of its own in the browser, so it may take
//!   as long as a job needs without holding up the app's frames. It has WASI and its libraries,
//!   but no gpu: it computes, and sends what it made back as bytes.
//! - The app starts it with `Worker::spawn` through the sandbox API (`be3:sandbox`), sends it
//!   messages with `send` and takes its answers with `receive`. Messages are typed: the worker's
//!   `Work::In` and `Work::Out`, which travel encoded with postcard, each whole and in order.
//!   Dropping the `Worker` stops it.
//! - Or, for work every editor of the app asks of one worker, each holds a `Shared`: they share
//!   one instance of the module, and each hears only the answers to what it asked, each with
//!   the ticket its job was given, and a ticket failed for each job a worker that stopped will
//!   never answer.
//!
//! Each message travels with the client that sent it or that it answers, which `Shared` tells its
//! editors apart by, and the job it is or answers, which `Shared` matches answers to tickets by:
//! a `Work` answers the client and job whose message it is taking, and `Worker` is client 0 and
//! job 0, as are the messages a worker sends as it starts.
//!
//! Nothing here assumes the guest has one thread: it keeps no state of its own, and a `Worker` is
//! used from the thread whose executor spawned it, as every op is.

use std::{collections::VecDeque, marker::PhantomData, rc::Rc};

mod shared;

pub use shared::{Shared, Ticket};

use serde::{Serialize, de::DeserializeOwned};

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

/// Why a worker's message was refused: it came without the client it answers, so the module is no
/// worker of this crate's (`worker!`). A broken protocol, not a stop: the worker may still be
/// running.
pub const UNFRAMED: &str = "a worker broke the worker protocol: its message came with no client \
                            (is the module exported with be_sdk_worker::worker!?)";

/// What a worker said, as `Raw::receive_framed` hears it.
enum Received {
    /// A message, and the client and job it answers.
    Message(Address, Vec<u8>),
    /// Why it stopped.
    Stopped(String),
    /// A message that came without its client (`UNFRAMED`).
    Unframed,
}

/// A message as it travels: postcard.
fn encode<T: Serialize>(message: &T) -> Vec<u8> {
    postcard::to_stdvec(message).expect("a worker message always encodes")
}

fn decode<T: DeserializeOwned>(message: &[u8], from: &str) -> Result<T, String> {
    postcard::from_bytes(message)
        .map_err(|error| format!("{from} sent a message that did not decode: {error}"))
}

/// A worker the app started: an instance of one of its worker modules, which takes `Job`s and
/// answers with `Answer`s (its `Work::In` and `Work::Out`).
pub struct Worker<Job, Answer> {
    raw: Raw,
    _messages: PhantomData<fn(Job) -> Answer>,
}

impl<Job: Serialize, Answer: DeserializeOwned> Worker<Job, Answer> {
    /// Starts `module`, one of the worker modules the app's manifest names under `workers`
    /// (such as `"tiles.wasm"`), as an instance of its own. One that cannot start, such as a
    /// module the app does not stage, says why from `receive`. Drop a `Worker` (and its
    /// `Sender`s) once `receive` says it stopped: a stopped worker stays held until then.
    pub fn spawn(module: &str) -> Self {
        Self {
            raw: Raw::spawn(module),
            _messages: PhantomData,
        }
    }

    /// Sends the worker one job. Jobs arrive in the order they were sent; one sent to a worker
    /// that stopped is dropped, and `receive` says why it stopped.
    pub fn send(&self, job: &Job) {
        self.raw.sender.send_as(Address::WORKER, &encode(job));
    }

    /// Something that sends to this worker, for a task that sends while another receives.
    pub fn sender(&self) -> Sender<Job> {
        Sender {
            raw: self.raw.sender.clone(),
            _jobs: PhantomData,
        }
    }

    /// The worker's next message, once it has sent one, or why it stopped: it could not start,
    /// trapped or panicked. Once it stopped, every later call fails the same way. A message that
    /// broke the protocol, or did not decode as an `Answer`, fails instead, and the next call
    /// goes on to the one after it.
    pub async fn receive(&mut self) -> Result<Answer, String> {
        match self.raw.receive_framed().await {
            Received::Message(_, message) => decode(&message, "a worker"),
            Received::Stopped(why) => Err(why),
            Received::Unframed => Err(UNFRAMED.to_owned()),
        }
    }
}

/// A worker instance as bytes, which `Worker` and `Shared` type.
struct Raw {
    sender: RawSender,
    received: VecDeque<Vec<u8>>,
}

impl Raw {
    fn spawn(module: &str) -> Self {
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
            sender: RawSender(Rc::new(Instance::new(source, config))),
            received: VecDeque::new(),
        }
    }

    /// The worker's next message and the client and job it answers (`Shared`).
    async fn receive_framed(&mut self) -> Received {
        loop {
            if let Some(frame) = self.received.pop_front() {
                return match unframe(&frame) {
                    Some((address, message)) => Received::Message(address, message.to_vec()),
                    None => Received::Unframed,
                };
            }
            let next = self.sender.0.next().await.map_err(|error| match error {
                OpError::Unsupported => NO_WORKERS.to_owned(),
                error => format!("the host did not answer: {error:?}"),
            });
            match next {
                Ok(Ok(frames)) => self.received.extend(frames),
                Ok(Err(why)) | Err(why) => return Received::Stopped(why),
            }
        }
    }
}

/// Sends jobs to a worker (`Worker::sender`). The worker stops once its `Worker` and every
/// `Sender` are dropped.
pub struct Sender<Job> {
    raw: RawSender,
    _jobs: PhantomData<fn(Job)>,
}

impl<Job> Clone for Sender<Job> {
    fn clone(&self) -> Self {
        Self {
            raw: self.raw.clone(),
            _jobs: PhantomData,
        }
    }
}

impl<Job: Serialize> Sender<Job> {
    /// Sends the worker one job (`Worker::send`).
    pub fn send(&self, job: &Job) {
        self.raw.send_as(Address::WORKER, &encode(job));
    }
}

#[derive(Clone)]
struct RawSender(Rc<Instance>);

impl RawSender {
    fn send_as(&self, address: Address, message: &[u8]) {
        self.0.send(vec![frame(address, message)]).detach();
    }
}

/// Whom a message between an app and its worker is from or for: the client (`Shared` gives each
/// editor its own; `Worker` is 0), and the job it is or answers (a `Shared` ticket; 0 otherwise).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Address {
    client: u64,
    job: u64,
}

impl Address {
    /// A `Worker`'s messages, and what a worker says as it starts.
    const WORKER: Self = Self { client: 0, job: 0 };
}

/// A message as it travels between an app and its worker: its address, then the message.
fn frame(address: Address, message: &[u8]) -> Vec<u8> {
    [
        &address.client.to_le_bytes()[..],
        &address.job.to_le_bytes()[..],
        message,
    ]
    .concat()
}

fn unframe(frame: &[u8]) -> Option<(Address, &[u8])> {
    let (client, rest) = frame.split_first_chunk::<8>()?;
    let (job, message) = rest.split_first_chunk::<8>()?;
    Some((
        Address {
            client: u64::from_le_bytes(*client),
            job: u64::from_le_bytes(*job),
        },
        message,
    ))
}

/// What a worker module does: its state, made when it starts, and how it takes each message from
/// the app that spawned it, an `In`, answering with `Out`s (the app's `Worker<In, Out>` or
/// `Shared<In, Out>`). Export it with `worker!`.
pub trait Work: Sized + 'static {
    /// What the app sends it.
    type In: DeserializeOwned;
    /// What it answers with.
    type Out: Serialize;

    /// Starts the worker.
    fn start(app: &mut App<Self::Out>) -> Result<Self, String>;

    /// Takes one message from the app. Whatever answers it is sent through `app` before this
    /// returns: `app` addresses it to the client and job that asked (`Shared`), and lasts only
    /// for this call, so a worker has no way to answer later. A `Shared` client takes the first
    /// answer to a job as its answer and drops any after; a job answered with none stays in
    /// flight until the worker stops. An error stops the worker, and the app's `receive` says
    /// why; so does a message that does not decode as an `In`.
    fn receive(&mut self, message: Self::In, app: &mut App<Self::Out>) -> Result<(), String>;
}

/// The app that spawned a worker, as the worker sees it.
pub struct App<'a, Out> {
    outbound: &'a mut Vec<Vec<u8>>,
    /// The client and job whose message the worker is taking, which what it sends answers.
    address: Address,
    _out: PhantomData<fn(Out)>,
}

impl<Out: Serialize> App<'_, Out> {
    /// Sends the app one message, which its `Worker::receive` answers with: an answer to the
    /// client and job whose message the worker is taking (`Shared`), or to client 0 as it starts.
    pub fn send(&mut self, message: &Out) {
        self.outbound.push(frame(self.address, &encode(message)));
    }
}

/// Starts `W`, which sends what it says as it starts to `outbound`.
fn start<W: Work>(outbound: &mut Vec<Vec<u8>>) -> Result<W, String> {
    W::start(&mut App {
        outbound,
        address: Address::WORKER,
        _out: PhantomData,
    })
}

/// Hands `W` each of `frames`, as its app sent them, answering to `outbound`.
fn step<W: Work>(
    work: &mut W,
    frames: Vec<Vec<u8>>,
    outbound: &mut Vec<Vec<u8>>,
) -> Result<(), String> {
    for frame in frames {
        let (address, message) = unframe(&frame).ok_or(
            "the app broke the worker protocol: its message came with no client (send through \
             be_sdk_worker's Worker or Shared)",
        )?;
        let message = decode(message, "the app")?;
        work.receive(
            message,
            &mut App {
                outbound,
                address,
                _out: PhantomData,
            },
        )?;
    }
    Ok(())
}

/// A `Work` run in the caller's own instance rather than one of its own: how an app's tests
/// serve its workers (be-sdk-test's `BeuiTest::serve_worker`), and a worker crate's tests drive
/// it, in step with the test. Frames in and out are as an instance of the worker module takes and
/// sends them.
pub struct Hosted<W>(W);

impl<W: Work> Hosted<W> {
    /// Starts `W`, which sends what it says as it starts to `outbound`.
    pub fn start(outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        start::<W>(outbound).map(Self)
    }

    /// Hands the worker the frames its app sent, which it answers to `outbound`. An error stops
    /// it, as it stops an instance.
    pub fn step(
        &mut self,
        frames: Vec<Vec<u8>>,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<(), String> {
        step(&mut self.0, frames, outbound)
    }

    /// Sends the worker one message as a `Worker` would, and answers what it sent back: how a
    /// worker crate's tests ask it something.
    pub fn ask(&mut self, message: &W::In) -> Result<Vec<W::Out>, String>
    where
        W::In: Serialize,
        W::Out: DeserializeOwned,
    {
        let mut outbound = Vec::new();
        self.step(
            vec![frame(Address::WORKER, &encode(message))],
            &mut outbound,
        )?;
        outbound
            .iter()
            .map(|frame| {
                let (_, message) = unframe(frame).ok_or_else(|| UNFRAMED.to_owned())?;
                decode(message, "the worker")
            })
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
