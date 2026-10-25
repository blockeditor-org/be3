use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use be_store::FileStore;

use super::{
    Config,
    worker::{Inbox, Shared, serve},
};

pub(super) struct Running;

impl Running {
    pub(super) fn finish(&mut self) {}
}

pub(super) fn spawn(
    config: Config,
    inbox: Inbox,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
) -> Running {
    let directory = config.data_dir.join("be-objects");
    let store = move || {
        FileStore::open(&directory)
            .map_err(|error| format!("the local object store cannot open: {error}"))
    };
    wasm_bindgen_futures::spawn_local(serve(
        config,
        store,
        inbox,
        shared,
        changed,
    ));
    Running
}

pub(super) fn spawn_local(task: impl Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(task);
}

pub(crate) async fn sleep(duration: Duration) {
    let Some(window) = web_sys::window() else {
        return std::future::pending().await;
    };
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            &resolve,
            duration.as_millis() as i32,
        );
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

pub(super) fn await_flush(sealed: std::sync::mpsc::Receiver<()>) {
    let _ = sealed;
}
