use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use crate::current::{try_with_document, with_document};
use ::reactive::{Stored, on_cleanup, store};

pub struct TimerState {
    due: Cell<Option<Instant>>,
    work: Box<dyn Fn() -> Option<Duration>>,
}

impl TimerState {
    pub fn due(&self) -> Option<Instant> {
        self.due.get()
    }

    pub fn fire(&self, now: Instant) {
        if self.due.get().is_none_or(|due| due > now) {
            return;
        }
        self.due.set(None);
        if let Some(delay) = (self.work)() {
            self.due.set(Some(now + delay));
        }
    }
}

#[derive(Clone, Copy)]
pub struct Timer(Stored<TimerState>);

impl Timer {
    pub fn start(&self, delay: Duration) {
        let Some(state) = self.0.try_get() else {
            return;
        };
        let due = now() + delay;
        if state.due.get().is_some_and(|held| held <= due) {
            return;
        }
        state.due.set(Some(due));
        try_with_document(|document| document.request_repaint_after(delay));
    }

    pub fn restart(&self, delay: Duration) {
        self.stop();
        self.start(delay);
    }

    pub fn stop(&self) {
        self.0.try_with(|state| state.due.set(None));
    }

    pub fn running(&self) -> bool {
        self.0
            .try_with(|state| state.due.get().is_some())
            .unwrap_or(false)
    }
}

pub fn now() -> Instant {
    try_with_document(|document| document.now()).unwrap_or_else(Instant::now)
}

pub fn create_timer(work: impl Fn() -> Option<Duration> + 'static) -> Timer {
    let state = store(TimerState {
        due: Cell::new(None),
        work: Box::new(work),
    });
    with_document(|document| document.register_timer(Rc::downgrade(&state.get())));
    on_cleanup(move || {
        state.try_with(|state| state.due.set(None));
    });
    Timer(state)
}

pub type Timers = Vec<Weak<TimerState>>;
