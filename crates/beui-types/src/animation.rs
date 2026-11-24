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

/// A spring pulling a displacement back to 0. It never oscillates: it is critically damped or
/// overdamped, a `damping` below critical (`2 * sqrt(stiffness)`) acting as critical, so a
/// motion let go at rest comes to its end without passing it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Spring {
    pub stiffness: f32,
    pub damping: f32,
}

impl Spring {
    /// The critically damped spring of `stiffness`: the quickest that never passes its end.
    pub fn critical(stiffness: f32) -> Self {
        Self {
            stiffness,
            damping: 2.0 * stiffness.max(0.0).sqrt(),
        }
    }

    /// Whether it rests without passing its end from rest: critically damped or overdamped.
    pub fn never_oscillates(self) -> bool {
        self.damping * self.damping >= 4.0 * self.stiffness - 1e-3
    }

    pub fn advance(self, displacement: &mut f32, velocity: &mut f32, seconds: f32) {
        if seconds <= 0.0 {
            return;
        }
        let x = *displacement;
        let v = *velocity;
        let stiffness = self.stiffness.max(0.0);
        let decay = (self.damping * 0.5).max(stiffness.sqrt());
        let discriminant = decay * decay - stiffness;
        let (next, speed) = if discriminant < 1e-4 {
            let b = v + decay * x;
            let envelope = (-decay * seconds).exp();
            (
                envelope * (x + b * seconds),
                envelope * (v - decay * b * seconds),
            )
        } else {
            let root = discriminant.sqrt();
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
