use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Condvar, PoisonError},
};

use be_wasm_threads::Mutex;

use be_block_store::FileStore;
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

/// Told, or dropped, when the stack started last has ended: the next waits for it, so two
/// stacks never serve the same files at once. Only the page starts stacks, so only it locks this.
static LAST_ENDED: std::sync::Mutex<Option<tokio::sync::oneshot::Receiver<()>>> =
    std::sync::Mutex::new(None);

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
    let (ended, ending) = tokio::sync::oneshot::channel();
    let previous = LAST_ENDED
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .replace(ending);
    let entry: Entry = Box::new(move || {
        Box::pin(async move {
            if let Some(previous) = previous {
                let _ = previous.await;
            }
            let directory = config.data_dir.join("be-objects");
            let store = move || {
                FileStore::open(&directory)
                    .map_err(|error| format!("the local object store cannot open: {error}"))
            };
            serve(config, store, inbox, shared, changed).await;
            let _ = ended.send(());
        })
    });
    let entry = Box::into_raw(Box::new(entry)) as usize;
    // unsafe(ffi)
    unsafe { be_worker_start(entry) };
    Running
}

#[wasm_bindgen]
pub fn be_worker_run(entry: usize) -> js_sys::Promise {
    be_wasm_threads::initialize_worker();
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
