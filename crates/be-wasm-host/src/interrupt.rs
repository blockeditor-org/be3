//! Stopping a host's guests from any thread: every isolate an app runs on (its own and each of
//! its threads') is terminated, which stops a guest at its next call or loop, a thread blocked in
//! `memory.atomic.wait` included.

use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

use crate::threads::Spawner;

#[derive(Default)]
pub(crate) struct Interrupt {
    interrupted: AtomicBool,
    apps: Mutex<Vec<Weak<Spawner>>>,
}

#[derive(Clone)]
pub struct Interrupter(Arc<Interrupt>);

impl Interrupt {
    pub(crate) fn new() -> Arc<Self> {
        Arc::default()
    }

    pub(crate) fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    pub(crate) fn watch(&self, app: &Arc<Spawner>) {
        if let Ok(mut apps) = self.apps.lock() {
            apps.retain(|app| app.strong_count() > 0);
            apps.push(Arc::downgrade(app));
        }
    }
}

impl Interrupter {
    pub(crate) fn new(interrupt: &Arc<Interrupt>) -> Self {
        Self(Arc::clone(interrupt))
    }

    pub fn interrupt(&self) {
        if self.0.interrupted.swap(true, Ordering::SeqCst) {
            return;
        }
        let apps: Vec<Arc<Spawner>> = match self.0.apps.lock() {
            Ok(apps) => apps.iter().filter_map(Weak::upgrade).collect(),
            Err(_) => return,
        };
        for app in apps {
            app.terminate(true);
        }
    }
}
