//! How much linear memory each running guest holds, for be-host's memory statistics. A wasm
//! memory only grows, so this is each guest's peak heap.

use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicU64, Ordering},
};

pub(crate) struct Gauge {
    name: String,
    bytes: AtomicU64,
}

static LIVE: Mutex<Vec<Weak<Gauge>>> = Mutex::new(Vec::new());

impl Gauge {
    pub(crate) fn register(name: String) -> Arc<Self> {
        let gauge = Arc::new(Self {
            name,
            bytes: AtomicU64::new(0),
        });
        let mut live = LIVE.lock().unwrap_or_else(|held| held.into_inner());
        live.retain(|gauge| gauge.strong_count() > 0);
        live.push(Arc::downgrade(&gauge));
        gauge
    }

    pub(crate) fn set(&self, bytes: u64) {
        self.bytes.store(bytes, Ordering::Relaxed);
    }
}

/// Each running guest's name (the name its thread was spawned with) and the bytes of linear
/// memory it holds, in the order they started.
pub fn memories() -> Vec<(String, u64)> {
    let mut live = LIVE.lock().unwrap_or_else(|held| held.into_inner());
    live.retain(|gauge| gauge.strong_count() > 0);
    live.iter()
        .filter_map(Weak::upgrade)
        .map(|gauge| (gauge.name.clone(), gauge.bytes.load(Ordering::Relaxed)))
        .collect()
}
