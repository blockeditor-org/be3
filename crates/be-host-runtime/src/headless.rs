//! Runs the runtime with no beui: no window, no presenter and no gpu, stepping the apps when they
//! wake rather than when a display asks for a frame (guides/apps/runtime.md, "Headless"). `be` runs
//! host components this way; be-host, driven by beui, starts each frame the same way
//! (`begin_host_frame`).

use std::{
    sync::{Arc, Condvar, Mutex, PoisonError},
    time::{Duration, Instant, SystemTime},
};

use be_host_base::host::{self, Frame};

/// Starts a host frame: what the block stack's worker handed over since the last one, then the
/// frame's clock and the letters waiting for it. Every frame of the runtime begins here, under
/// beui or headless.
pub fn begin_host_frame(frame: Frame) {
    be_host_stack::take_updates_in_frame();
    host::start(frame);
}

/// The runtime with nothing on screen. One per thread, which owns the runtime's state.
pub struct Headless {
    wakes: Arc<(Mutex<u64>, Condvar)>,
}

/// How many times the host was woken: one count for the process, as the host's waker is
/// installed once (`host::install_waker`), so a second `Headless` (a test after another, in one
/// process) is woken too. Each `run_until` waits for the count to move past what it was before
/// its step, so none takes another's wake, and a wake during a step is not lost.
fn wakes() -> Arc<(Mutex<u64>, Condvar)> {
    static WAKES: std::sync::OnceLock<Arc<(Mutex<u64>, Condvar)>> = std::sync::OnceLock::new();
    Arc::clone(WAKES.get_or_init(|| {
        let wakes = Arc::new((Mutex::new(0), Condvar::new()));
        let waking = Arc::clone(&wakes);
        host::install_waker(move || {
            *waking.0.lock().unwrap_or_else(PoisonError::into_inner) += 1;
            waking.1.notify_all();
        });
        wakes
    }))
}

impl Headless {
    pub fn start() -> Self {
        host::install_clock(Instant::now);
        crate::install_headless();
        Self { wakes: wakes() }
    }

    /// One step: the letters that woke the host delivered, the apps polled, their calls served
    /// and what they laid out settled. Answers when the host asks to step again of its own
    /// accord, if it does.
    pub fn step(&mut self) -> Option<Duration> {
        begin_host_frame(Frame {
            now: Some(Instant::now()),
            wall: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .ok(),
            seen: false,
            ..Frame::default()
        });
        crate::begin_frame(None, None, None);
        crate::poll();
        crate::flush();
        crate::settle();
        let output = host::finish();
        match output.repaint || output.animating {
            true => Some(Duration::ZERO),
            false => output.repaint_after,
        }
    }

    /// Steps until `done` answers, sleeping between steps until something wakes the host or
    /// it asked to step again, for at most `within`.
    pub fn run_until<T>(
        &mut self,
        within: Duration,
        mut done: impl FnMut() -> Option<T>,
    ) -> Option<T> {
        let deadline = Instant::now() + within;
        let wakes = Arc::clone(&self.wakes);
        let (count, ringing) = &*wakes;
        loop {
            let before = *count.lock().unwrap_or_else(PoisonError::into_inner);
            let again = self.step();
            if let Some(done) = done() {
                return Some(done);
            }
            let left = deadline.checked_duration_since(Instant::now())?;
            let wait = again.map_or(left, |again| again.min(left));
            let woken = count.lock().unwrap_or_else(PoisonError::into_inner);
            if *woken == before {
                drop(
                    ringing
                        .wait_timeout_while(woken, wait, |woken| *woken == before)
                        .unwrap_or_else(PoisonError::into_inner),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests;
