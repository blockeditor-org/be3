mod fling;
pub use fling::Fling;
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use crate::current::with_document;
use crate::timer::{Timer, create_timer, now};
use ::reactive::{Get, Stored, create_effect, on_cleanup, store};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Motion {
    #[default]
    Animated,
    Instant,
}

impl Motion {
    pub fn animates(self) -> bool {
        self == Self::Animated
    }
}

pub fn motion() -> Motion {
    with_document(|document| document.motion())
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
        running: with_document(|document| document.animations()),
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
    let preference = with_document(|document| document.watch_motion());
    create_effect(move || {
        if !preference.get().animates() {
            animation.finish();
        }
    });
    on_cleanup(move || animation.stop());
    animation
}

#[derive(Clone, Copy, Debug)]
pub enum Easing {
    Linear,
    OutQuad,
    OutCubic,
}

impl Easing {
    pub fn sample(self, progress: f32) -> f32 {
        let progress = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => progress,
            Self::OutQuad => 1.0 - (1.0 - progress).powi(2),
            Self::OutCubic => 1.0 - (1.0 - progress).powi(3),
        }
    }
}

pub fn tween(elapsed: f32, duration: f32, easing: Easing) -> f32 {
    easing.sample(if duration <= 0.0 {
        1.0
    } else {
        elapsed / duration
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    pub fn advance(self, displacement: &mut f32, velocity: &mut f32, seconds: f32) {
        if seconds <= 0.0 {
            return;
        }
        let x = *displacement;
        let v = *velocity;
        let decay = self.damping * 0.5;
        let discriminant = self.stiffness - decay * decay;
        let (next, speed) = if discriminant.abs() < 1e-4 {
            let b = v + decay * x;
            let envelope = (-decay * seconds).exp();
            (
                envelope * (x + b * seconds),
                envelope * (v - decay * b * seconds),
            )
        } else if discriminant > 0.0 {
            let frequency = discriminant.sqrt();
            let (sin, cos) = (frequency * seconds).sin_cos();
            let b = (v + decay * x) / frequency;
            let envelope = (-decay * seconds).exp();
            let next = envelope * (x * cos + b * sin);
            (
                next,
                envelope * frequency * (-x * sin + b * cos) - decay * next,
            )
        } else {
            let root = (-discriminant).sqrt();
            let first = -decay + root;
            let second = -decay - root;
            let a = (v - second * x) / (first - second);
            let b = x - a;
            let one = a * (first * seconds).exp();
            let two = b * (second * seconds).exp();
            (one + two, first * one + second * two)
        };
        *displacement = next;
        *velocity = speed;
        if next.abs() < 0.25 && speed.abs() < 5.0 {
            *displacement = 0.0;
            *velocity = 0.0;
        }
    }
}
