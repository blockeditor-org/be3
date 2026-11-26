//! Stopping a host's guests from any thread: the isolate each app runs on is terminated, which
//! stops the guest at its next call or loop.

use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
pub(crate) struct Interrupt {
    interrupted: AtomicBool,
    apps: Mutex<Vec<Weak<v8::IsolateHandle>>>,
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

    /// Terminates `app`'s isolate when the host is interrupted.
    pub(crate) fn watch(&self, app: &Arc<v8::IsolateHandle>) {
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
        let apps: Vec<Arc<v8::IsolateHandle>> = match self.0.apps.lock() {
            Ok(apps) => apps.iter().filter_map(Weak::upgrade).collect(),
            Err(_) => return,
        };
        for app in apps {
            app.terminate_execution();
        }
    }
}
