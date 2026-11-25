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

/// A spring pulling a displacement back to 0. It swings past 0 at most once: the first time it
/// passes 0 it may overshoot (an underdamped spring, `damping` below `2 * sqrt(stiffness)`),
/// and from then on, `passed`, it moves as a critically damped spring at least, landing on 0 on
/// its way back without passing it again. It never rings.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    /// Moves `displacement` and `velocity` on by `seconds`. `passed` says it has passed 0
    /// already; it starts false for each new motion (a release, a new target), the spring sets
    /// it as it passes 0, and clears it at rest.
    pub fn advance(self, displacement: &mut f32, velocity: &mut f32, passed: &mut bool, seconds: f32) {
        if seconds <= 0.0 {
            return;
        }
        let x = *displacement;
        let v = *velocity;
        let stiffness = self.stiffness.max(0.0);
        let decay = match *passed {
            true => (self.damping * 0.5).max(stiffness.sqrt()),
            false => self.damping * 0.5,
        };
        let discriminant = stiffness - decay * decay;
        let (mut next, mut speed) = if discriminant.abs() < 1e-4 {
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
        if next * x < 0.0 || (x != 0.0 && next == 0.0) {
            match *passed {
                // Passing 0 a second time, it lands there instead.
                true => (next, speed) = (0.0, 0.0),
                false => *passed = true,
            }
        }
        if next.abs() < 0.25 && speed.abs() < 5.0 {
            next = 0.0;
            speed = 0.0;
        }
        if next == 0.0 && speed == 0.0 {
            *passed = false;
        }
        *displacement = next;
        *velocity = speed;
    }
}
