#[cfg(target_arch = "wasm32")]
use crate::Deadline;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::sync::{Condvar, Mutex};

    use crate::Deadline;

    static RUNG: Mutex<u64> = Mutex::new(0);
    static RINGING: Condvar = Condvar::new();

    pub fn rung() -> u64 {
        *RUNG.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn ring() {
        let mut rung = RUNG.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *rung = rung.wrapping_add(1);
        RINGING.notify_all();
    }

    pub fn wait(since: u64, deadline: Deadline) -> bool {
        let mut rung = RUNG.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        while *rung == since {
            let Some(left) = deadline.remaining() else {
                return false;
            };
            rung = RINGING
                .wait_timeout(rung, left)
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .0;
        }
        true
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::{ring, rung, wait};

#[cfg(target_arch = "wasm32")]
pub fn rung() -> u64 {
    0
}

#[cfg(target_arch = "wasm32")]
pub fn ring() {}

#[cfg(target_arch = "wasm32")]
pub fn wait(_since: u64, _deadline: Deadline) -> bool {
    false
}
