use std::{
    any::Any,
    future::Future,
    sync::{
        Arc, Condvar, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
};

use be_wasm_threads::Mutex;

use be_block_store::FileStore;

use super::worker::{Inbox, Shared, Start, serve};

// unsafe(ffi)
#[link(wasm_import_module = "wasi")]
unsafe extern "C" {
    #[link_name = "be-worker-start"]
    fn be_worker_start(entry: *const u8, entry_length: usize, bytes: *const u8, length: usize);
}

/// Starts the work named `entry` in the host's worker (`web/be-worker.js`), as a future beside
/// the block stacks it runs, from `bytes`: what it needs, as a worker whose memory the page does
/// not share would be sent it. `web/threads.js` copies both out before this returns and posts
/// them to the worker, where be-host's `be_worker_run` starts the entry by its name. What reads
/// files, which the page may not, runs this way: the stacks (`STACK_ENTRY`) and be-host's
/// `host_files`.
pub fn start_in_worker(entry: &str, bytes: Vec<u8>) {
    // unsafe(ffi)
    unsafe { be_worker_start(entry.as_ptr(), entry.len(), bytes.as_ptr(), bytes.len()) };
}

/// What a worker's entry is started with that is not bytes yet: the channels and shared
/// handles it and the page talk through, which are values in the heap they share. The page
/// leaves them here and sends their number in the entry's bytes, and the entry takes them
/// (plans/web_page_never_waits.md: each becomes a message port once the worker has a memory of
/// its own).
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Transports(u64);

static LEFT: Mutex<Vec<(u64, Box<dyn Any + Send>)>> = Mutex::new(Vec::new());
static NEXT_LEFT: AtomicU64 = AtomicU64::new(1);

impl Transports {
    /// Leaves `transports` for the entry whose bytes carry the number answered.
    pub fn leave<T: Send + 'static>(transports: T) -> Self {
        let number = NEXT_LEFT.fetch_add(1, Ordering::Relaxed);
        LEFT.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((number, Box::new(transports)));
        Self(number)
    }

    /// Takes what was left under this number, which the page left as a `T`: anything else is
    /// a bug.
    pub fn take<T: 'static>(self) -> T {
        let mut left = LEFT.lock().unwrap_or_else(PoisonError::into_inner);
        let at = left
            .iter()
            .position(|(number, _)| *number == self.0)
            .expect("a worker's transports are taken once");
        let (_, transports) = left.swap_remove(at);
        drop(left);
        *transports
            .downcast()
            .expect("a worker's transports are what its page left")
    }
}

/// The name a stack starts under in the host's worker (`run_stack`).
pub const STACK_ENTRY: &str = "stack";

/// What a stack's worker starts from, as its bytes.
#[derive(serde::Deserialize, serde::Serialize)]
struct StackStart {
    start: Start,
    transports: Transports,
}

/// What a stack's worker and the page talk through (`Transports`).
struct StackTransports {
    inbox: Inbox,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
    /// Ends when the stack started before this one has: a stack waits for it, so two stacks
    /// never serve the same files at once.
    previous: Option<tokio::sync::oneshot::Receiver<()>>,
    /// Told, or dropped, when this stack has ended.
    ended: tokio::sync::oneshot::Sender<()>,
}

/// Told, or dropped, when the stack started last has ended (`StackTransports::previous`). A
/// trap (panic=abort) ends the worker without dropping it, so the next stack never starts after
/// one, as before. Only the page starts stacks, so only it locks this.
static LAST_ENDED: std::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>> =
    std::sync::Mutex::new(None);

pub(super) struct Running;

impl Running {
    pub(super) fn stop(&mut self) {}
}

pub(super) fn wait_for_stopped() {}

pub(super) fn spawn(
    start: Start,
    inbox: Inbox,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
) -> Running {
    let (ended, ending) = tokio::sync::oneshot::channel();
    let previous = LAST_ENDED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .replace(ending);
    let transports = Transports::leave(StackTransports {
        inbox,
        shared,
        changed,
        previous,
        ended,
    });
    let bytes = postcard::to_stdvec(&StackStart { start, transports })
        .expect("what a stack starts from encodes");
    start_in_worker(STACK_ENTRY, bytes);
    Running
}

/// In the host's worker: runs the stack whose bytes the page sent (`spawn`).
pub async fn run_stack(bytes: Vec<u8>) {
    // The page encoded them from a `StackStart`: bytes that do not decode are a bug.
    let StackStart { start, transports } =
        postcard::from_bytes(&bytes).expect("what a stack starts from decodes");
    let StackTransports {
        inbox,
        shared,
        changed,
        previous,
        ended,
    } = transports.take();
    if let Some(previous) = previous {
        let _ = previous.await;
    }
    let directory = start.config.data_dir.join("be-objects");
    let store = move || {
        FileStore::open(&directory)
            .map_err(|error| format!("the local object store cannot open: {error}"))
    };
    serve(start, store, inbox, shared, changed).await;
    let _ = ended.send(());
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
