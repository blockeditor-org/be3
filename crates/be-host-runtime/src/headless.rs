//! Runs the runtime with no beui: no window, no presenter and no gpu, stepping the apps when
//! they wake rather than when a display asks for a frame (plans/headless_apps.md). `be` runs
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
    woken: Arc<(Mutex<bool>, Condvar)>,
}

/// What wakes a `Headless`: one for the process, as the host's waker is installed once
/// (`host::install_waker`), so a second `Headless` (a test after another, in one process) is
/// woken too rather than sleeping until its wait runs out.
fn woken() -> Arc<(Mutex<bool>, Condvar)> {
    static WOKEN: std::sync::OnceLock<Arc<(Mutex<bool>, Condvar)>> = std::sync::OnceLock::new();
    Arc::clone(WOKEN.get_or_init(|| {
        let woken = Arc::new((Mutex::new(true), Condvar::new()));
        let waking = Arc::clone(&woken);
        host::install_waker(move || {
            *waking.0.lock().unwrap_or_else(PoisonError::into_inner) = true;
            waking.1.notify_all();
        });
        woken
    }))
}

impl Headless {
    pub fn start() -> Self {
        let woken = woken();
        // Its first step runs at once.
        *woken.0.lock().unwrap_or_else(PoisonError::into_inner) = true;
        host::install_clock(Instant::now);
        crate::install_headless();
        Self { woken }
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
        loop {
            let again = self.step();
            if let Some(done) = done() {
                return Some(done);
            }
            let left = deadline.checked_duration_since(Instant::now())?;
            let wait = again.map_or(left, |again| again.min(left));
            let (woken, ringing) = &*self.woken;
            let mut woken = woken.lock().unwrap_or_else(PoisonError::into_inner);
            if !*woken {
                woken = ringing
                    .wait_timeout(woken, wait)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0;
            }
            *woken = false;
        }
    }
}

#[cfg(test)]
mod tests;
