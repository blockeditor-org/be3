use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

use wasmtime::{Engine, Store, UpdateDeadline};

use crate::threads::{Spawner, Spawns};

pub(crate) struct Interrupt {
    engine: Engine,
    interrupted: AtomicBool,
    plugins: Mutex<Vec<Weak<Spawner>>>,
}

#[derive(Clone)]
pub struct Interrupter(Arc<Interrupt>);

impl Interrupt {
    pub(crate) fn new(engine: Engine) -> Arc<Self> {
        Arc::new(Self {
            engine,
            interrupted: AtomicBool::new(false),
            plugins: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn interrupted(&self) -> bool {
        self.interrupted.load(Ordering::SeqCst)
    }

    pub(crate) fn tick(&self) {
        self.engine.increment_epoch();
    }

    pub(crate) fn watch(&self, plugin: &Arc<Spawner>) {
        if let Ok(mut plugins) = self.plugins.lock() {
            plugins.retain(|plugin| plugin.strong_count() > 0);
            plugins.push(Arc::downgrade(plugin));
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
        let plugins: Vec<Arc<Spawner>> = match self.0.plugins.lock() {
            Ok(plugins) => plugins.iter().filter_map(Weak::upgrade).collect(),
            Err(_) => return,
        };
        for plugin in plugins {
            plugin.reap();
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
