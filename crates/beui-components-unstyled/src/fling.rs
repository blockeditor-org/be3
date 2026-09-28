#[cfg(test)]
mod tests;

use std::sync::OnceLock;

const INFLEXION: f32 = 0.35;
const START_TENSION: f32 = 0.5;
const END_TENSION: f32 = 1.0;
const SAMPLES: usize = 100;
const FRICTION: f32 = 0.015;
const GRAVITY: f32 = 9.806_65;
const INCHES_PER_METRE: f32 = 39.37;
const POINTS_PER_INCH: f32 = 160.0;
const TUNING: f32 = 0.84;
const PHYSICAL: f32 = GRAVITY * INCHES_PER_METRE * POINTS_PER_INCH * TUNING;

fn deceleration_rate() -> f32 {
    0.78_f32.ln() / 0.9_f32.ln()
}

fn spline() -> &'static [f32; SAMPLES + 1] {
    static SPLINE: OnceLock<[f32; SAMPLES + 1]> = OnceLock::new();
    SPLINE.get_or_init(|| {
        let first = START_TENSION * INFLEXION;
        let second = 1.0 - END_TENSION * (1.0 - INFLEXION);
        let mut positions = [1.0; SAMPLES + 1];
        let mut low = 0.0_f32;
        for (index, position) in positions.iter_mut().take(SAMPLES).enumerate() {
            let alpha = index as f32 / SAMPLES as f32;
            let mut high = 1.0_f32;
            let (x, coefficient) = loop {
                let x = low + (high - low) / 2.0;
                let coefficient = 3.0 * x * (1.0 - x);
                let time = coefficient * ((1.0 - x) * first + x * second) + x * x * x;
                if (time - alpha).abs() < 1e-5 || high - low < 1e-7 {
                    break (x, coefficient);
                }
                if time > alpha {
                    high = x;
                } else {
                    low = x;
                }
            };
            *position = coefficient * ((1.0 - x) * START_TENSION + x) + x * x * x;
        }
        positions
    })
}

fn spline_at(progress: f32) -> (f32, f32) {
    let positions = spline();
    let scaled = progress.clamp(0.0, 1.0) * SAMPLES as f32;
    let index = (scaled as usize).min(SAMPLES - 1);
    let (from, to) = (positions[index], positions[index + 1]);
    let slope = (to - from) * SAMPLES as f32;
    (from + (scaled - index as f32) * (to - from), slope)
}

#[derive(Clone, Copy, Debug)]
pub struct Fling {
    distance: f32,
    duration: f32,
    elapsed: f32,
    travelled: f32,
}

impl Fling {
    pub fn new(velocity: f32) -> Option<Self> {
        if velocity == 0.0 {
            return None;
        }
        let rate = deceleration_rate();
        let exponent = (INFLEXION * velocity.abs() / (FRICTION * PHYSICAL)).ln();
        let duration = (exponent / (rate - 1.0)).exp();
        let distance = FRICTION * PHYSICAL * (rate / (rate - 1.0) * exponent).exp();
        (duration > 0.0 && distance > 0.0).then_some(Self {
            distance: distance.copysign(velocity),
            duration,
            elapsed: 0.0,
            travelled: 0.0,
        })
    }

    pub fn distance(&self) -> f32 {
        self.distance
    }

    pub fn duration(&self) -> f32 {
        self.duration
    }

    pub fn done(&self) -> bool {
        self.elapsed >= self.duration
    }

    pub fn velocity(&self) -> f32 {
        if self.done() {
            return 0.0;
        }
        let (_, slope) = spline_at(self.elapsed / self.duration);
        self.distance * slope / self.duration
    }

    pub fn advance(&mut self, elapsed: f32) -> f32 {
        self.elapsed = (self.elapsed + elapsed).min(self.duration);
        let (position, _) = spline_at(self.elapsed / self.duration);
        let travelled = self.distance * position;
        travelled - std::mem::replace(&mut self.travelled, travelled)
    }
}
