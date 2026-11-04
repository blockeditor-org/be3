mod fling;
mod velocity;

pub use fling::Fling;
pub use velocity::Velocity;

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
