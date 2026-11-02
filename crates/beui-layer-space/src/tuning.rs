use std::f32::consts::FRAC_PI_2;
use std::time::{Duration, Instant};

use beui_types::Key;
use glam::Vec3;

use crate::head::Head;

pub const STEP: f32 = 5.0 * std::f32::consts::PI / 180.0;
pub const SHOWN_FOR: Duration = Duration::from_secs(3);
pub const BLEND_FROM: f32 = 40.0 * std::f32::consts::PI / 180.0;
pub const BLEND_TO: f32 = 70.0 * std::f32::consts::PI / 180.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Up {
    Sky,
    View,
    Blend { from: f32, to: f32 },
}

impl Up {
    pub fn weight(self, pitch: f32) -> f32 {
        match self {
            Up::Sky => 0.0,
            Up::View => 1.0,
            Up::Blend { from, to } => {
                let pitch = pitch.abs();
                if to <= from {
                    return if pitch >= from { 1.0 } else { 0.0 };
                }
                let t = ((pitch - from) / (to - from)).clamp(0.0, 1.0);
                t * t * (3.0 - 2.0 * t)
            }
        }
    }

    pub fn of(self, head: &Head) -> Vec3 {
        let forward = head.rotation() * Vec3::NEG_Z;
        let pitch = forward.y.clamp(-1.0, 1.0).asin();
        let weight = self.weight(pitch);
        (Vec3::Y * (1.0 - weight) + head.up() * weight).normalize_or(Vec3::Y)
    }

    fn next(self) -> Self {
        match self {
            Up::Sky => Up::View,
            Up::View => Up::Blend {
                from: BLEND_FROM,
                to: BLEND_TO,
            },
            Up::Blend { .. } => Up::Sky,
        }
    }

    fn nudged(self, from: f32, to: f32) -> Self {
        let (was_from, was_to) = match self {
            Up::Blend { from, to } => (from, to),
            _ => (BLEND_FROM, BLEND_TO),
        };
        let from = (was_from + from).clamp(0.0, FRAC_PI_2);
        let to = (was_to + to).clamp(from, FRAC_PI_2);
        Up::Blend { from, to }
    }

    pub fn describe(self) -> String {
        let degrees = |radians: f32| (radians.to_degrees()).round() as i32;
        match self {
            Up::Sky => "up is the sky".to_owned(),
            Up::View => "up is the top of the view".to_owned(),
            Up::Blend { from, to } => format!(
                "up turns from the sky to the top of the view between {} and {} degrees",
                degrees(from),
                degrees(to)
            ),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tuning {
    pub simulator: Up,
    pub headset: Up,
    changed: Option<(Instant, bool)>,
}

impl Default for Tuning {
    fn default() -> Self {
        Self {
            simulator: Up::Sky,
            headset: Up::Sky,
            changed: None,
        }
    }
}

impl Tuning {
    pub fn up(&self, headset: bool) -> Up {
        match headset {
            true => self.headset,
            false => self.simulator,
        }
    }

    pub fn key(&mut self, key: Key, repeat: bool, headset: bool, now: Option<Instant>) -> bool {
        let up = self.up(headset);
        let changed = match key {
            Key::M if !repeat => up.next(),
            Key::M => up,
            Key::ArrowLeft => up.nudged(-STEP, 0.0),
            Key::ArrowRight => up.nudged(STEP, 0.0),
            Key::ArrowDown => up.nudged(0.0, -STEP),
            Key::ArrowUp => up.nudged(0.0, STEP),
            _ => return false,
        };
        match headset {
            true => self.headset = changed,
            false => self.simulator = changed,
        }
        self.changed = now.map(|now| (now, headset));
        true
    }

    pub fn label(&self, now: Option<Instant>) -> Option<String> {
        let (changed, headset) = self.changed?;
        if now? >= changed + SHOWN_FOR {
            return None;
        }
        let place = match headset {
            true => "Headset mouse",
            false => "Simulator mouse",
        };
        Some(format!("{place}: {}", self.up(headset).describe()))
    }

    pub fn due_in(&self, now: Option<Instant>) -> Duration {
        match (self.changed, now) {
            (Some((changed, _)), Some(now)) if now < changed + SHOWN_FOR => {
                changed + SHOWN_FOR - now
            }
            _ => Duration::MAX,
        }
    }
}
