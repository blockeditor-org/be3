use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use reactive::{Get, ReadSignal, Set, Signal, Stored, create_signal, on_cleanup, store};

use crate::node::NodeId;

pub const LAYOUTS_PER_FRAME: usize = 4;

thread_local! {
    static CLOCK: Cell<Option<Instant>> = const { Cell::new(None) };
    static FROZEN: Cell<bool> = const { Cell::new(false) };
    static PINNED_WALL_CLOCK: Cell<Option<(Instant, Duration)>> = const { Cell::new(None) };
    static STARTED: Cell<bool> = const { Cell::new(false) };
    static PENDING: Cell<bool> = const { Cell::new(false) };
    static TIMERS: RefCell<Vec<Weak<TimerState>>> = RefCell::default();
    static LAYOUT: RefCell<Vec<Weak<dyn Fn() -> bool>>> = RefCell::default();
    static PLACED: RefCell<HashMap<NodeId, Signal<bool>>> = RefCell::default();
}

pub fn now() -> Instant {
    CLOCK.with(Cell::get).unwrap_or_else(Instant::now)
}

pub fn freeze_clock(frozen: bool) {
    FROZEN.with(|held| held.set(frozen));
    if frozen {
        CLOCK.with(|clock| clock.set(Some(clock.get().unwrap_or_else(Instant::now))));
    }
}

pub fn advance_clock(by: Duration) {
    CLOCK.with(|clock| clock.set(Some(clock.get().unwrap_or_else(Instant::now) + by)));
    request_frame();
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

pub struct TimerState {
    due: Cell<Option<Instant>>,
    work: Box<dyn Fn() -> Option<Duration>>,
    listed: Cell<bool>,
}

impl TimerState {
    fn unlist(&self) -> bool {
        self.listed.set(false);
        false
    }

    fn arm(self: &Rc<Self>, due: Instant) {
        self.due.set(Some(due));
        if !self.listed.replace(true) {
            TIMERS.with(|timers| timers.borrow_mut().push(Rc::downgrade(self)));
        }
    }

    fn fire(&self, now: Instant) {
        if self.due.get().is_none_or(|due| due > now) {
            return;
        }
        self.due.set(None);
        if let Some(delay) = (self.work)() {
            self.due.set(Some(now + delay));
            schedule_after(delay);
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
        schedule_after(delay);
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

pub fn create_timer(work: impl Fn() -> Option<Duration> + 'static) -> Timer {
    let state = store(TimerState {
        due: Cell::new(None),
        work: Box::new(work),
        listed: Cell::new(false),
    });
    on_cleanup(move || {
        state.try_with(|state| state.due.set(None));
    });
    Timer(state)
}

fn run_timers(now: Instant) {
    let due: Vec<Rc<TimerState>> = TIMERS.with(|timers| {
        let mut timers = timers.borrow_mut();
        timers.retain(|timer| {
            timer
                .upgrade()
                .is_some_and(|timer| timer.due.get().is_some() || timer.unlist())
        });
        timers
            .iter()
            .filter_map(Weak::upgrade)
            .filter(|timer| timer.due.get().is_some_and(|due| due <= now))
            .collect()
    });
    for timer in due {
        timer.fire(now);
    }
}

pub fn on_layout(pass: Rc<dyn Fn() -> bool>) {
    LAYOUT.with(|passes| passes.borrow_mut().push(Rc::downgrade(&pass)));
    on_cleanup(move || drop(pass));
    request_frame();
}

pub fn watch_placed(node: NodeId) -> ReadSignal<bool> {
    let held = PLACED.with(|placed| placed.borrow().get(&node).copied());
    let signal = match held {
        Some(signal) => signal,
        None => {
            let signal = match crate::document::owner(node) {
                Some(owner) => owner.run(|| create_signal(false)),
                None => create_signal(false),
            };
            PLACED.with(|placed| placed.borrow_mut().insert(node, signal));
            request_frame();
            signal
        }
    };
    signal.read_only()
}

fn layout_pass() -> bool {
    let passes: Vec<Rc<dyn Fn() -> bool>> = LAYOUT.with(|passes| {
        let mut passes = passes.borrow_mut();
        passes.retain(|pass| pass.strong_count() > 0);
        passes.iter().filter_map(Weak::upgrade).collect()
    });
    let mut changed = crate::measure::pass();
    for pass in passes {
        changed |= pass();
    }
    let placed: Vec<(NodeId, Signal<bool>)> = PLACED.with(|placed| {
        placed
            .borrow()
            .iter()
            .map(|(node, signal)| (*node, *signal))
            .collect()
    });
    for (node, signal) in placed {
        if !crate::document::contains(node) {
            PLACED.with(|placed| placed.borrow_mut().remove(&node));
            continue;
        }
        let connected = crate::dom::connected(node);
        if signal.get_untracked() != connected {
            signal.set(connected);
            changed = true;
        }
    }
    changed
}

pub fn frame() {
    PENDING.with(|pending| pending.set(false));
    if !FROZEN.with(Cell::get) {
        CLOCK.with(|clock| clock.set(Some(Instant::now())));
    }
    let now = now();
    run_timers(now);
    reactive::flush();
    for _ in 0..LAYOUTS_PER_FRAME {
        if !layout_pass() {
            break;
        }
        reactive::flush();
    }
    if !FROZEN.with(Cell::get) {
        CLOCK.with(|clock| clock.set(None));
    }
}

pub fn started() -> bool {
    STARTED.with(Cell::get)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn start() {
    STARTED.with(|started| started.set(true));
    reactive::wake_with(request_frame);
    request_frame();
}

pub fn request_frame() {
    if !started() || PENDING.with(|pending| pending.replace(true)) {
        return;
    }
    crate::dom::request_animation_frame(frame);
}

fn schedule_after(delay: Duration) {
    if !started() {
        return;
    }
    if delay.is_zero() {
        request_frame();
    } else {
        crate::dom::set_timeout(delay, request_frame);
    }
}
