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
    fn be_worker_start(
        entry: *const u8,
        entry_length: usize,
        transports: f64,
        bytes: *const u8,
        length: usize,
    );
}

/// Starts the work named `entry` in the host's worker (`web/be-worker.js`), as a future beside
/// the block stacks it runs, from `transports` and `bytes`: what it needs, as a worker whose
/// memory the page does not share would be sent it. `web/threads.js` copies the name and the
/// bytes out before this returns and posts them, with the transports' number, to the worker,
/// where be-host's `be_worker_run` starts the entry by its name. What reads files, which the
/// page may not, runs this way: the stacks (`STACK_ENTRY`) and be-host's `host_files`.
pub fn start_in_worker(entry: &str, transports: Transports, bytes: Vec<u8>) {
    // The worker takes them, or drops them if it has no such entry (`Transports::arrived`).
    let transports = std::mem::ManuallyDrop::new(transports);
    // unsafe(ffi)
    unsafe {
        be_worker_start(
            entry.as_ptr(),
            entry.len(),
            transports.0 as f64,
            bytes.as_ptr(),
            bytes.len(),
        )
    };
}

/// What a worker's entry is started with that is not bytes yet: the channels and shared
/// handles it and the page talk through, which are values in the heap they share. The page
/// leaves them here and sends their number beside the entry's bytes, and the entry takes them
/// (plans/web_page_never_waits.md: each becomes a message port once the worker has a memory of
/// its own). Dropped untaken, they go: a worker with no entry by the name sent drops them, so
/// what waits on them on the page hears the worker stopped. A worker that ends before it gets
/// that far (its module fails to load) strands them, and the page waits on for good.
pub struct Transports(u64);

static LEFT: Mutex<Vec<(u64, Box<dyn Any + Send>)>> = Mutex::new(Vec::new());
static NEXT_LEFT: AtomicU64 = AtomicU64::new(1);

impl Transports {
    /// Leaves `transports` for the entry started with what this answers (`start_in_worker`).
    pub fn leave<T: Send + 'static>(transports: T) -> Self {
        let number = NEXT_LEFT.fetch_add(1, Ordering::Relaxed);
        LEFT.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((number, Box::new(transports)));
        Self(number)
    }

    /// In the host's worker: the transports whose number the page sent (`start_in_worker`).
    pub fn arrived(number: f64) -> Self {
        Self(number as u64)
    }

    /// Takes what was left under this number, which the page left as a `T`: anything else is
    /// a bug.
    pub fn take<T: 'static>(self) -> T {
        let transports = self.remove().expect("a worker's transports are taken once");
        *transports
            .downcast()
            .expect("a worker's transports are what its page left")
    }

    fn remove(&self) -> Option<Box<dyn Any + Send>> {
        let mut left = LEFT.lock().unwrap_or_else(PoisonError::into_inner);
        let at = left.iter().position(|(number, _)| *number == self.0)?;
        Some(left.swap_remove(at).1)
    }
}

impl Drop for Transports {
    fn drop(&mut self) {
        // Dropped once the lock is let go of: what they hold may take it.
        drop(self.remove());
    }
}

/// The name a stack starts under in the host's worker (`run_stack`).
pub const STACK_ENTRY: &str = "stack";

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
    let bytes = postcard::to_stdvec(&start).expect("what a stack starts from encodes");
    start_in_worker(STACK_ENTRY, transports, bytes);
    Running
}

/// In the host's worker: runs the stack whose bytes the page sent (`spawn`).
pub async fn run_stack(transports: Transports, bytes: Vec<u8>) {
    // The page encoded them from a `Start`: bytes that do not decode are a bug.
    let start: Start = postcard::from_bytes(&bytes).expect("what a stack starts from decodes");
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
