use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    sync::{
        Arc, Condvar,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
};

use be_wasm_threads::Mutex;

use be_block_store::FileStore;
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{MessageEvent, MessagePort};

use super::{
    seen::Handover,
    worker::{Inbox, Shared, Start, serve},
};

#[wasm_bindgen]
extern "C" {
    /// `web/threads.js`'s: posts `entry`, a copy of `bytes` and one end of a new message
    /// channel to the host's worker, which starts it (be-host's `be_worker_run`), and answers
    /// the other end.
    #[wasm_bindgen(js_name = beWorkerStart)]
    fn be_worker_start(entry: &str, bytes: &[u8]) -> MessagePort;
}

/// Starts the work named `entry` in the host's worker (`web/be-worker.js`), as a future beside
/// the block stacks it runs, from `bytes`: what it needs, as a worker whose memory the page does
/// not share is sent it. Answers the page's end of the port the two talk through after. be-host's
/// `be_worker_run` starts the entry by its name. What reads files, which the page may not, runs
/// this way: the stacks (`STACK_ENTRY`) and be-host's `host_files`.
pub fn start_in_worker(entry: &str, bytes: Vec<u8>) -> Port {
    Port::new(be_worker_start(entry, &bytes))
}

/// One end of a message channel between the page and an entry in the host's worker
/// (`start_in_worker`): each message is bytes, copied out of the memory of the side that posts
/// it. A port never posts empty bytes but as it is dropped, which tells the other end it went
/// (`listen`'s `ended`): an entry the worker does not know drops its port, so what the page
/// waits on hears the worker stopped. A worker that fails (a trap) drops nothing, so
/// `web/threads.js` tells each of the page's ends instead, as it hears the worker's `error`.
pub struct Port {
    port: MessagePort,
    heard: Option<Closure<dyn FnMut(MessageEvent)>>,
}

impl Port {
    /// An end of a channel to the other side: the page's that `start_in_worker` answers, or
    /// the worker's that came with the entry.
    pub fn new(port: MessagePort) -> Self {
        Self { port, heard: None }
    }

    pub fn post(&self, bytes: &[u8]) {
        debug_assert!(
            !bytes.is_empty(),
            "a port posts no empty message but its end"
        );
        self.post_bytes(bytes);
    }

    fn post_bytes(&self, bytes: &[u8]) {
        let array = js_sys::Uint8Array::from(bytes);
        let _ = self
            .port
            .post_message_with_transferable(&array, &js_sys::Array::of1(&array.buffer()));
    }

    /// Hands each message that arrives to `heard`, in the order posted, and calls `ended` once
    /// the other end is dropped. Messages arrive as tasks of this thread's event loop.
    pub fn listen(
        &mut self,
        mut heard: impl FnMut(Vec<u8>) + 'static,
        ended: impl FnOnce() + 'static,
    ) {
        let mut ended = Some(ended);
        let closure = Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            let bytes = js_sys::Uint8Array::new(&event.data()).to_vec();
            match bytes.is_empty() {
                true => {
                    if let Some(ended) = ended.take() {
                        ended();
                    }
                }
                false => heard(bytes),
            }
        });
        self.port
            .set_onmessage(Some(closure.as_ref().unchecked_ref()));
        self.heard = Some(closure);
    }
}

impl Drop for Port {
    fn drop(&mut self) {
        self.post_bytes(&[]);
        self.port.set_onmessage(None);
        self.port.close();
    }
}

/// The name a stack starts under in the host's worker (`run_stack`).
pub const STACK_ENTRY: &str = "stack";

/// What the page posts a stack's worker.
#[derive(serde::Deserialize, serde::Serialize)]
enum ToWorker {
    /// A command, as its bytes (`worker::CommandPost`).
    Command(#[serde(with = "serde_bytes")] Vec<u8>),
    Nudge,
    /// The page took this many more updates, of these many bytes (`Handover::acknowledge`).
    Took(usize, usize),
}

/// What a stack's worker posts the page.
#[derive(serde::Deserialize, serde::Serialize)]
enum ToPage {
    /// An update's bytes (`seen::Handed`), and whether it goes in place of those before
    /// (`seen::put_in_place`).
    Update(#[serde(with = "serde_bytes")] Vec<u8>, bool),
    /// Wake the host though no update came (`Handover::wake_page`).
    Wake,
}

fn post<T: serde::Serialize>(port: &Port, message: &T) {
    port.post(
        &postcard::to_stdvec(message).expect("a message between the page and its worker encodes"),
    );
}

fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> T {
    // The other side encoded it from a `T`: bytes that do not decode are a bug.
    postcard::from_bytes(bytes).expect("a message between the page and its worker decodes")
}

pub(super) struct Running;

impl Running {
    pub(super) fn stop(&mut self) {}
}

pub(super) fn wait_for_stopped() {}

/// Starts a stack in the host's worker from `start`. The page keeps its ends of `inbox`'s
/// channels: a task copies what the page sends on them to the stack's port, and what the worker
/// hands over arrives on the page's handover (`Handover::put`), which wakes the host.
pub(super) fn spawn(start: Start, inbox: Inbox) -> Running {
    let bytes = postcard::to_stdvec(&start).expect("what a stack starts from encodes");
    let mut port = start_in_worker(STACK_ENTRY, bytes);
    let handover = Arc::clone(&inbox.handover);
    let gone = Arc::clone(&inbox.handover);
    port.listen(
        move |bytes| {
            match decode(&bytes) {
                ToPage::Update(bytes, false) => handover.put(bytes),
                ToPage::Update(bytes, true) => crate::seen::put_in_place(&handover, bytes),
                ToPage::Wake => {}
            }
            be_host_base::host::wake();
        },
        // The worker's end went while the page still runs the stack: it failed, or ended by
        // itself. What the page reads says so (`Handover::gone`), from an update that tells
        // every watcher.
        move || {
            gone.worker_gone();
            let told = crate::seen::Told {
                changes: crate::Changes::everything(),
                ..Default::default()
            };
            gone.put(postcard::to_stdvec(&told.handed()).expect("an update encodes"));
            be_host_base::host::wake();
        },
    );
    spawn_local(to_worker(port, inbox));
    Running
}

/// On the page: posts the stack's worker each command, nudge and take, in the order made, until
/// the page stops the stack, when the port goes and tells the worker so.
async fn to_worker(port: Port, inbox: Inbox) {
    let Inbox {
        mut commands,
        mut nudged,
        handover,
        mut stopped,
    } = inbox;
    let mut told = (0, 0);
    let mut nudging = true;
    std::future::poll_fn(|context| {
        let closed = loop {
            match commands.0.poll_recv(context) {
                Poll::Ready(Some(bytes)) => post(&port, &ToWorker::Command(bytes)),
                Poll::Ready(None) => break true,
                Poll::Pending => break false,
            }
        };
        while nudging {
            match nudged.poll_changed(context) {
                Poll::Ready(Ok(())) => post(&port, &ToWorker::Nudge),
                Poll::Ready(Err(_)) => nudging = false,
                Poll::Pending => break,
            }
        }
        while let Poll::Ready((updates, bytes)) = handover.poll_took(context, &mut told) {
            post(&port, &ToWorker::Took(updates, bytes));
        }
        match closed || Pin::new(&mut stopped).poll(context).is_ready() {
            true => Poll::Ready(()),
            false => Poll::Pending,
        }
    })
    .await;
    while let Ok(bytes) = commands.0.try_recv() {
        post(&port, &ToWorker::Command(bytes));
    }
}

thread_local! {
    /// In the worker: ends, told or dropped, when the stack started last has ended. The next
    /// waits for it, so two stacks never serve the same files at once. A trap (panic=abort)
    /// ends the worker without dropping it, so the next stack never starts after one.
    static LAST_ENDED: RefCell<Option<tokio::sync::oneshot::Receiver<()>>> =
        const { RefCell::new(None) };
}

/// In the host's worker: runs the stack the page started (`spawn`) from `bytes`, talking to the
/// page through `port`. The worker keeps its own ends of the stack's channels and its own
/// `Shared`: what the page posts goes onto them, and what the stack hands over is posted to the
/// page (`to_page`).
pub async fn run_stack(port: Port, bytes: Vec<u8>) {
    let start: Start = decode(&bytes);
    let (ended, ending) = tokio::sync::oneshot::channel();
    let previous = LAST_ENDED.with_borrow_mut(|last| last.replace(ending));
    let (commands_post, commands) = super::worker::command_channel();
    let (nudges, nudged) = super::nudge::channel();
    let (stopping, stopped) = tokio::sync::oneshot::channel::<()>();
    let handover = Arc::new(Handover::default());
    let sent = Arc::new(AtomicUsize::new(0));
    let mut port = port;
    // Dropped as the page's port goes: the stack then stops as it does natively when the page
    // drops its ends (`Stack::drop`).
    let from_page = Rc::new(RefCell::new(Some((commands_post, nudges, stopping))));
    port.listen(
        {
            let (from_page, handover, sent) = (
                Rc::clone(&from_page),
                Arc::clone(&handover),
                Arc::clone(&sent),
            );
            move |bytes| match decode(&bytes) {
                ToWorker::Command(bytes) => {
                    if let Some((commands, ..)) = from_page.borrow().as_ref() {
                        sent.fetch_add(1, Ordering::AcqRel);
                        let _ = commands.0.send(bytes);
                    }
                }
                ToWorker::Nudge => {
                    if let Some((_, nudges, _)) = from_page.borrow().as_ref() {
                        nudges.nudge();
                    }
                }
                ToWorker::Took(updates, bytes) => handover.acknowledge(updates, bytes),
            }
        },
        move || drop(from_page.borrow_mut().take()),
    );
    let port = Rc::new(port);
    spawn_local(to_page(Rc::clone(&port), Arc::clone(&handover)));
    if let Some(previous) = previous {
        let _ = previous.await;
    }
    if !crate::discovery::loaded() {
        crate::discovery::load().await;
    }
    let shared = Arc::new(Mutex::new(Shared {
        wall: super::worker::Wall::new(start.config.clock.clone()),
        sent,
        ..Shared::default()
    }));
    let directory = start.config.data_dir.join("be-objects");
    let store = move || {
        FileStore::open(&directory)
            .map_err(|error| format!("the local object store cannot open: {error}"))
    };
    let inbox = Inbox {
        commands,
        nudged,
        handover: Arc::clone(&handover),
        stopped,
    };
    serve(start, store, inbox, shared, Arc::new(Condvar::new())).await;
    // `to_page` posts what is left and ends.
    handover.close();
    let _ = ended.send(());
}

/// In the worker: posts the page each update the stack hands over, and a wake asked for with
/// none, until the stack ends.
async fn to_page(port: Rc<Port>, handover: Arc<Handover>) {
    let mut handed = 0;
    loop {
        handover.handed_past(handed).await;
        handed = handover.handed();
        let updates = handover.drain();
        let went = !updates.is_empty();
        for (bytes, replaces) in updates {
            post(&port, &ToPage::Update(bytes, replaces));
        }
        if handover.take_wake() && !went {
            post(&port, &ToPage::Wake);
        }
        if handover.closed() {
            return;
        }
    }
}

pub(super) fn spawn_local(task: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(task);
}

thread_local! {
    static FIRST: std::cell::RefCell<Vec<Box<dyn FnMut()>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// On the host's worker, has `work` run at the start of each pass of a stack's loop, ahead of
/// the pass's own work: the host's files are kept then (be-host's `host_files`) if the worker
/// has not yielded since, so a change to them waits at most for the synchronous stretch it
/// arrived in.
pub fn run_first(work: impl FnMut() + 'static) {
    FIRST.with_borrow_mut(|first| first.push(Box::new(work)));
}

/// Runs what `run_first` asked for.
pub(super) fn first() {
    FIRST.with(|first| {
        for work in first.borrow_mut().iter_mut() {
            work();
        }
    });
}

pub(super) fn await_flush(sealed: std::sync::mpsc::Receiver<usize>) -> Option<usize> {
    let _ = sealed;
    None
}
