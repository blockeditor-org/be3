use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::core::reactive::{Timer, create_timer, now};
use crate::reactive::{Get, Stored, create_effect, on_cleanup, store};

pub use crate::types::{Easing, Fling, Motion, Spring, Velocity, tween};

pub fn motion() -> Motion {
    crate::core::animation::motion()
}

#[derive(Clone, Copy, Debug)]
pub struct AnimationFrame {
    pub now: Instant,
    pub elapsed: Duration,
    pub delta: Duration,
}

struct State {
    active: Cell<bool>,
    running: Rc<Cell<usize>>,
    generation: Cell<u64>,
    started: Cell<Instant>,
    stepped: Cell<Instant>,
    timer: Cell<Option<Timer>>,
    step: Box<dyn Fn(AnimationFrame) -> bool>,
    finish: Box<dyn Fn()>,
}

#[derive(Clone, Copy)]
pub struct Animation(Stored<State>);

impl Animation {
    pub fn start(self) {
        let Some(state) = self.0.try_value() else {
            return;
        };
        if state.active.replace(true) {
            return;
        }
        state.running.set(state.running.get() + 1);
        state.generation.set(state.generation.get().wrapping_add(1));
        state.started.set(now());
        state.stepped.set(now());
        if !motion().animates() {
            self.finish();
            return;
        }
        state.timer.get().unwrap().start(Duration::ZERO);
    }

    pub fn stop(self) {
        self.0.try_with(|state| {
            if state.active.replace(false) {
                state.running.set(state.running.get() - 1);
            }
            state.generation.set(state.generation.get().wrapping_add(1));
            if let Some(timer) = state.timer.get() {
                timer.stop();
            }
        });
    }

    pub fn finish(self) {
        let Some(state) = self.0.try_value() else {
            return;
        };
        if !state.active.get() {
            return;
        }
        self.stop();
        (state.finish)();
    }

    pub fn running(self) -> bool {
        self.0.try_with(|state| state.active.get()).unwrap_or(false)
    }
}

pub fn create_animation(
    step: impl Fn(AnimationFrame) -> bool + 'static,
    finish: impl Fn() + 'static,
) -> Animation {
    let state = store(State {
        active: Cell::new(false),
        running: crate::core::animation::running(),
        generation: Cell::new(0),
        started: Cell::new(now()),
        stepped: Cell::new(now()),
        timer: Cell::new(None),
        step: Box::new(step),
        finish: Box::new(finish),
    });
    let animation = Animation(state);
    let timer = create_timer(move || {
        let state = state.value();
        if !state.active.get() {
            return None;
        }
        let generation = state.generation.get();
        let now = now();
        let frame = AnimationFrame {
            now,
            elapsed: now.saturating_duration_since(state.started.get()),
            delta: now.saturating_duration_since(state.stepped.replace(now)),
        };
        let again = (state.step)(frame);
        if state.generation.get() == generation {
            if again {
                state.timer.get().unwrap().start(Duration::ZERO);
            } else {
                animation.finish();
            }
        }
        None
    });
    state.with(|state| state.timer.set(Some(timer)));
    let preference = crate::core::animation::watch_motion();
    create_effect(move || {
        if !preference.get().animates() {
            animation.finish();
        }
    });
    on_cleanup(move || animation.stop());
    animation
}
