mod fling;
mod velocity;

pub use fling::Fling;
pub use velocity::Velocity;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// A spring pulling a displacement back to 0. A `SpringMotion` moves on it, swinging past 0 at
/// most once: underdamped (a `damping` below `2 * sqrt(stiffness)`) it overshoots, and from the
/// moment it passes 0 it moves critically damped at least, landing there on the way back. It
/// never rings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
}

/// How near 0, and how slow, a motion is taken to be at rest.
const REST_DISTANCE: f32 = 0.25;
const REST_SPEED: f32 = 5.0;

impl Spring {
    /// Moves `displacement` and `velocity` on by `seconds` with no memory of passing 0, for a
    /// motion that stops the moment it reaches its target (a page sliding off, content against
    /// its edge). Anything that may pass its target is a `SpringMotion`.
    pub fn step(self, displacement: &mut f32, velocity: &mut f32, seconds: f32) {
        if seconds <= 0.0 {
            return;
        }
        let (x, v) = self.evolve(*displacement, *velocity, self.decay(false), seconds);
        (*displacement, *velocity) = at_rest(x, v);
    }

    fn decay(self, passed: bool) -> f32 {
        let decay = self.damping.max(0.0) * 0.5;
        match passed {
            true => decay.max(self.stiffness.max(0.0).sqrt()),
            false => decay,
        }
    }

    /// Where a motion from `x` at `v` is after `seconds`, damped by `decay`.
    fn evolve(self, x: f32, v: f32, decay: f32, seconds: f32) -> (f32, f32) {
        let stiffness = self.stiffness.max(0.0);
        let discriminant = stiffness - decay * decay;
        if discriminant.abs() < 1e-4 {
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
            let (first, second) = (-decay + root, -decay - root);
            let a = (v - second * x) / (first - second);
            let b = x - a;
            let one = a * (first * seconds).exp();
            let two = b * (second * seconds).exp();
            (one + two, first * one + second * two)
        }
    }

    /// How long a motion from `x` at `v`, damped by `decay`, takes to first reach 0 after
    /// now, if it ever does.
    fn first_zero(self, x: f32, v: f32, decay: f32) -> Option<f32> {
        let stiffness = self.stiffness.max(0.0);
        let discriminant = stiffness - decay * decay;
        let time = if discriminant.abs() < 1e-4 {
            // e^(-decay t) (x + (v + decay x) t)
            let slope = v + decay * x;
            (slope != 0.0).then(|| -x / slope)?
        } else if discriminant > 0.0 {
            // e^(-decay t) R cos(frequency t - phase)
            let frequency = discriminant.sqrt();
            let b = (v + decay * x) / frequency;
            let angle = (b.atan2(x) + std::f32::consts::FRAC_PI_2).rem_euclid(std::f32::consts::PI);
            let angle = match angle <= 1e-6 {
                true => std::f32::consts::PI,
                false => angle,
            };
            angle / frequency
        } else {
            let root = (-discriminant).sqrt();
            let (first, second) = (-decay + root, -decay - root);
            let a = (v - second * x) / (first - second);
            let b = x - a;
            let ratio = -b / a;
            (a != 0.0 && ratio > 0.0).then(|| ratio.ln() / (first - second))?
        };
        (time.is_finite() && time > 0.0).then_some(time)
    }
}

fn at_rest(x: f32, v: f32) -> (f32, f32) {
    match x.abs() < REST_DISTANCE && v.abs() < REST_SPEED {
        true => (0.0, 0.0),
        false => (x, v),
    }
}

/// A motion on a `Spring`: how far it is from where it rests and how fast it moves, and whether
/// it has passed its rest since it set out, so it swings past at most once.
#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct SpringMotion {
    pub displacement: f32,
    pub velocity: f32,
    passed: bool,
}

impl SpringMotion {
    pub const fn new(displacement: f32, velocity: f32) -> Self {
        Self {
            displacement,
            velocity,
            passed: false,
        }
    }

    pub fn moving(&self) -> bool {
        self.displacement != 0.0 || self.velocity != 0.0
    }

    /// Sets out for a new rest `displacement` away, keeping its speed: it may pass this one
    /// once.
    pub fn retarget(&mut self, displacement: f32) {
        self.displacement = displacement;
        self.passed = false;
    }

    /// Let go at `velocity` (a gesture's release): it may pass its rest once.
    pub fn release(&mut self, velocity: f32) {
        self.velocity = velocity;
        self.passed = false;
    }

    /// Moves on by `seconds` on `spring`; whether it still moves. The moment it passes its
    /// rest, within the step, it goes on critically damped at least, and one that would pass
    /// it again lands there instead, so the motion is the same at any frame rate.
    pub fn advance(&mut self, spring: Spring, seconds: f32) -> bool {
        if seconds <= 0.0 {
            return self.moving();
        }
        let (mut x, mut v) = (self.displacement, self.velocity);
        let decay = spring.decay(self.passed);
        match spring.first_zero(x, v, decay).filter(|at| *at <= seconds) {
            Some(_) if self.passed => (x, v) = (0.0, 0.0),
            Some(at) => {
                let (_, speed) = spring.evolve(x, v, decay, at);
                self.passed = true;
                (x, v) = spring.evolve(0.0, speed, spring.decay(true), seconds - at);
            }
            None => (x, v) = spring.evolve(x, v, decay, seconds),
        }
        (x, v) = at_rest(x, v);
        if (x, v) == (0.0, 0.0) {
            self.passed = false;
        }
        self.displacement = x;
        self.velocity = v;
        self.moving()
    }
}
