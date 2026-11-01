use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::current::{try_with_document, with_document};
use ::reactive::{Stored, on_cleanup, store};

pub struct TimerState {
    due: Cell<Option<Instant>>,
    work: Box<dyn Fn() -> Option<Duration>>,
    armed: Rc<RefCell<Timers>>,
    listed: Cell<bool>,
}

impl TimerState {
    pub fn due(&self) -> Option<Instant> {
        self.due.get()
    }

    pub(crate) fn unlist(&self) -> bool {
        self.listed.set(false);
        false
    }

    fn arm(self: &Rc<Self>, due: Instant) {
        self.due.set(Some(due));
        if !self.listed.replace(true) {
            self.armed.borrow_mut().push(Rc::downgrade(self));
        }
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
        let Some(state) = self.0.try_value() else {
            return;
        };
        let due = now() + delay;
        if state.due.get().is_some_and(|held| held <= due) {
            return;
        }
        state.arm(due);
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

thread_local! {
    static FRAME_CLOCK: Cell<Option<Instant>> = const { Cell::new(None) };
}

pub(crate) fn set_frame_clock(now: Instant) {
    FRAME_CLOCK.with(|clock| clock.set(Some(now)));
}

pub(crate) fn frame_clock() -> Instant {
    FRAME_CLOCK.with(|clock| {
        let now = clock.get().unwrap_or_else(Instant::now);
        clock.set(Some(now));
        now
    })
}

pub fn now() -> Instant {
    try_with_document(|document| document.now()).unwrap_or_else(frame_clock)
}

pub const TEST_WALL_CLOCK: Duration = Duration::from_secs(1_772_444_460);

thread_local! {
    static PINNED_WALL_CLOCK: Cell<Option<(Instant, Duration)>> = const { Cell::new(None) };
}

pub fn pin_wall_clock(at: Option<(Instant, Duration)>) {
    PINNED_WALL_CLOCK.with(|held| held.set(at));
}

pub fn wall_clock() -> Duration {
    match PINNED_WALL_CLOCK.with(Cell::get) {
        Some((at, since_epoch)) => since_epoch + now().saturating_duration_since(at),
        None => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default(),
    }
}

pub fn create_timer(work: impl Fn() -> Option<Duration> + 'static) -> Timer {
    let armed = with_document(|document| document.timers());
    let state = store(TimerState {
        due: Cell::new(None),
        work: Box::new(work),
        armed,
        listed: Cell::new(false),
    });
    on_cleanup(move || {
        state.try_with(|state| state.due.set(None));
    });
    Timer(state)
}

pub type Timers = Vec<Weak<TimerState>>;
