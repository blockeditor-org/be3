use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use be_store::MemoryStore;

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
    wasm_bindgen_futures::spawn_local(serve(
        config,
        || Ok(MemoryStore::new()),
        inbox,
        shared,
        changed,
    ));
    Running
}

pub(super) fn refs(_: &Config) -> Result<be_client::RefsDb, String> {
    refs_in(std::path::Path::new(""))
}

pub(crate) fn refs_in(_: &std::path::Path) -> Result<be_client::RefsDb, String> {
    thread_local! {
        static MEMORY: be_client::RefsDb = be_client::RefsDb::memory();
    }
    Ok(MEMORY.with(Clone::clone))
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
