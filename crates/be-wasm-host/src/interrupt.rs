use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

use wasmtime::{Engine, Store, UpdateDeadline};

use crate::threads::{Spawner, Spawns};

pub(crate) struct Interrupt {
    engine: Engine,
    interrupted: AtomicBool,
    apps: Mutex<Vec<Weak<Spawner>>>,
}

#[derive(Clone)]
pub struct Interrupter(Arc<Interrupt>);

impl Interrupt {
    pub(crate) fn new(engine: Engine) -> Arc<Self> {
        Arc::new(Self {
            engine,
            interrupted: AtomicBool::new(false),
            apps: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    pub(crate) fn tick(&self) {
        self.engine.increment_epoch();
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
        self.0.tick();
        let apps: Vec<Arc<Spawner>> = match self.0.apps.lock() {
            Ok(apps) => apps.iter().filter_map(Weak::upgrade).collect(),
            Err(_) => return,
        };
        for app in apps {
            app.reap();
        }
    }
}

pub(crate) fn watch<T: Spawns + 'static>(store: &mut Store<T>, main: bool) {
    store.epoch_deadline_callback(move |context| {
        Ok(match context.data().spawner().halts(main) {
            true => UpdateDeadline::Interrupt,
            false => UpdateDeadline::Continue(1),
        })
    });
    store.set_epoch_deadline(1);
    if store.data().spawner().halts(main) {
        store.set_epoch_deadline(0);
    }
}
