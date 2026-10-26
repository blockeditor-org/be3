use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Condvar,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use wasi_threads::Mutex;

use be_store::FileStore;
use wasm_bindgen::{JsValue, prelude::wasm_bindgen};

use super::{
    Config,
    worker::{Inbox, Shared, serve},
};

type Entry = Box<dyn FnOnce() -> Pin<Box<dyn Future<Output = ()>>> + Send>;

// unsafe(ffi)
#[link(wasm_import_module = "wasi")]
unsafe extern "C" {
    #[link_name = "be-worker-start"]
    fn be_worker_start(entry: usize);
}

static STARTED: AtomicU64 = AtomicU64::new(0);
static FINISHED: AtomicU64 = AtomicU64::new(0);
const AWAITING_PREVIOUS: Duration = Duration::from_millis(5);

pub(super) struct Running;

impl Running {
    pub(super) fn stop(&mut self) {}
}

pub(super) fn wait_for_stopped() {}

pub(super) fn spawn(
    config: Config,
    inbox: Inbox,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
) -> Running {
    let turn = STARTED.fetch_add(1, Ordering::SeqCst);
    let entry: Entry = Box::new(move || {
        Box::pin(async move {
            while FINISHED.load(Ordering::SeqCst) < turn {
                config.clock.sleep(AWAITING_PREVIOUS).await;
            }
            let directory = config.data_dir.join("be-objects");
            let store = move || {
                FileStore::open(&directory)
                    .map_err(|error| format!("the local object store cannot open: {error}"))
            };
            serve(config, store, inbox, shared, changed).await;
            FINISHED.fetch_add(1, Ordering::SeqCst);
        })
    });
    let entry = Box::into_raw(Box::new(entry)) as usize;
    // unsafe(ffi)
    unsafe { be_worker_start(entry) };
    Running
}

#[wasm_bindgen]
pub fn be_worker_run(entry: usize) -> js_sys::Promise {
    wasi_threads::initialize_main_thread();
    // unsafe(memory)
    let entry = unsafe { Box::from_raw(entry as *mut Entry) };
    wasm_bindgen_futures::future_to_promise(async move {
        entry().await;
        Ok(JsValue::UNDEFINED)
    })
}

pub(super) fn spawn_local(task: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(task);
}

pub(super) fn await_flush(sealed: std::sync::mpsc::Receiver<()>) -> bool {
    let _ = sealed;
    false
}
