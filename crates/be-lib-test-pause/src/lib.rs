//! Places a test can hold code at, between two awaits, so it can change something there and
//! force a race that otherwise lands there only by chance. A crate calls `point` only in its
//! own tests (`#[cfg(test)]` at the call), so nothing of this runs in a build that ships.
//!
//! Each place is named, and keyed (a block's id, say) so tests running side by side in one
//! process hold only their own: `hold` it, wait until the code is there (`reached`), change what
//! the race needs, then `release` it.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};

use tokio::sync::watch;

struct Gate {
    /// How many times code came to the place while it was held.
    reached: watch::Sender<u64>,
    open: watch::Sender<bool>,
}

type Place = (&'static str, u128);

static GATES: LazyLock<Mutex<HashMap<Place, Gate>>> = LazyLock::new(Mutex::default);

fn gates() -> MutexGuard<'static, HashMap<Place, Gate>> {
    GATES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Code passes `name` for `key`: it waits there while a test holds it.
pub async fn point(name: &'static str, key: u128) {
    let mut open = {
        let gates = gates();
        let Some(gate) = gates.get(&(name, key)) else {
            return;
        };
        gate.reached.send_modify(|reached| *reached += 1);
        gate.open.subscribe()
    };
    let _ = open.wait_for(|open| *open).await;
}

/// Holds code that comes to `name` for `key` there until `release`.
pub fn hold(name: &'static str, key: u128) {
    gates().insert(
        (name, key),
        Gate {
            reached: watch::channel(0).0,
            open: watch::channel(false).0,
        },
    );
}

/// Waits until code has come to `name` for `key`, held, `times` times since `hold`.
pub async fn reached(name: &'static str, key: u128, times: u64) {
    let mut reached = gates()
        .get(&(name, key))
        .expect("the place is held")
        .reached
        .subscribe();
    let _ = reached.wait_for(|reached| *reached >= times).await;
}

/// Lets what waits at `name` for `key` go on, and what comes there later pass.
pub fn release(name: &'static str, key: u128) {
    if let Some(gate) = gates().remove(&(name, key)) {
        gate.open.send_replace(true);
    }
}
