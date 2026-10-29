use std::{
    sync::{Arc, Condvar, PoisonError},
    thread::{self, JoinHandle},
    time::Duration,
};

use wasi_threads::Mutex;

use be_store::FileStore;

use super::{
    Config,
    worker::{Inbox, Shared, serve},
};

const FLUSH_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) struct Running(Option<JoinHandle<()>>);

fn stopping() -> std::sync::MutexGuard<'static, Vec<JoinHandle<()>>> {
    static STOPPING: std::sync::Mutex<Vec<JoinHandle<()>>> = std::sync::Mutex::new(Vec::new());
    STOPPING.lock().unwrap_or_else(PoisonError::into_inner)
}

pub(super) fn wait_for_stopped() {
    let workers = std::mem::take(&mut *stopping());
    for worker in workers {
        let _ = worker.join();
    }
}

impl Running {
    pub(super) fn stop(&mut self) {
        if let Some(worker) = self.0.take() {
            stopping().push(worker);
        }
    }

    #[cfg(any(test, feature = "testing"))]
    pub(super) fn abandon(&mut self) {
        self.0 = None;
    }
}

pub(super) fn spawn(
    config: Config,
    inbox: Inbox,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
) -> Running {
    let previous = std::mem::take(&mut *stopping());
    let worker = thread::Builder::new()
        .name("block-app-be".into())
        .spawn(move || {
            for worker in previous {
                let _ = worker.join();
            }
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

pub(super) fn await_flush(sealed: std::sync::mpsc::Receiver<()>) -> bool {
    let _ = sealed.recv_timeout(FLUSH_TIMEOUT);
    true
}
