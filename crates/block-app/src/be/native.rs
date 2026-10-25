use std::{
    sync::{Arc, Condvar, Mutex, PoisonError},
    thread::{self, JoinHandle},
    time::Duration,
};

use be_store::FileStore;

use super::{
    Config,
    worker::{Inbox, Shared, serve},
};

const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct Running(Option<JoinHandle<()>>);

impl Running {
    pub(super) fn finish(&mut self) {
        if let Some(worker) = self.0.take() {
            let _ = worker.join();
        }
    }
}

pub(super) fn spawn(
    config: Config,
    inbox: Inbox,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
) -> Running {
    let worker = thread::Builder::new()
        .name("block-app-be".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    shared.lock().unwrap_or_else(PoisonError::into_inner).error =
                        Some(format!("the new block stack cannot start: {error}"));
                    return;
                }
            };
            let directory = config.data_dir.join("be-objects");
            let store = move || {
                FileStore::open(&directory)
                    .map_err(|error| format!("the local object store cannot open: {error}"))
            };
            tokio::task::LocalSet::new()
                .block_on(&runtime, serve(config, store, inbox, shared, changed));
        })
        .ok();
    Running(worker)
}

pub(super) fn spawn_local(task: impl Future<Output = ()> + 'static) {
    tokio::task::spawn_local(task);
}

pub(crate) async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

pub(super) fn await_flush(sealed: std::sync::mpsc::Receiver<()>) {
    let _ = sealed.recv_timeout(FLUSH_TIMEOUT);
}
