use std::cell::Cell;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use crate::reactive::{on_cleanup, try_with_document, with_document};

pub(crate) struct TimerState {
    due: Cell<Option<Instant>>,
    work: Box<dyn Fn() -> Option<Duration>>,
}

impl TimerState {
    pub(crate) fn due(&self) -> Option<Instant> {
        self.due.get()
    }

    pub(crate) fn fire(&self, now: Instant) {
        if self.due.get().is_none_or(|due| due > now) {
            return;
        }
        self.due.set(None);
        if let Some(delay) = (self.work)() {
            self.due.set(Some(now + delay));
        }
    }
}

#[derive(Clone)]
pub struct Timer(Rc<TimerState>);

impl Timer {
    pub fn start(&self, delay: Duration) {
        let due = Instant::now() + delay;
        if self.0.due.get().is_some_and(|held| held <= due) {
            return;
        }
        self.0.due.set(Some(due));
        try_with_document(|document| document.request_repaint_after(delay));
    }

    pub fn restart(&self, delay: Duration) {
        self.0.due.set(None);
        self.start(delay);
    }

    pub fn stop(&self) {
        self.0.due.set(None);
    }

    pub fn running(&self) -> bool {
        self.0.due.get().is_some()
    }
}

pub fn create_timer(work: impl Fn() -> Option<Duration> + 'static) -> Timer {
    let state = Rc::new(TimerState {
        due: Cell::new(None),
        work: Box::new(work),
    });
    with_document(|document| document.register_timer(Rc::downgrade(&state)));
    let held = Rc::clone(&state);
    on_cleanup(move || {
        held.due.set(None);
        drop(held);
    });
    Timer(state)
}

pub(crate) type Timers = Vec<Weak<TimerState>>;
