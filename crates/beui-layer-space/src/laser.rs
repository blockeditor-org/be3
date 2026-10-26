use std::time::Instant;

use beui_core::geometry::Pos2;
use beui_core::input::TouchId;
use beui_core::runner::{Hand, HandPose};
use beui_core::window::WindowId;
use glam::{Quat, Vec3};

use crate::head::{Head, Ray};
use crate::panel::Pose;
use crate::space::Held;

pub const DEVICE: u64 = 0x1a5e_0000;
pub const PRESS: f32 = 0.6;
pub const LET_GO: f32 = 0.4;
pub const FREEZE: f32 = 0.02;
pub const REACH: f32 = 5.0;
pub const MIN_CUTOFF: f32 = 1.0;
pub const BETA: f32 = 8.0;
pub const DERIVATIVE_CUTOFF: f32 = 1.0;
pub const JUMP: f32 = 0.15;
const SETTLED: f32 = 1e-5;

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct OneEuro {
    value: Option<Vec3>,
    derivative: Vec3,
    at: Option<Instant>,
}

fn smoothing(cutoff: f32, seconds: f32) -> f32 {
    let tau = 1.0 / (std::f32::consts::TAU * cutoff);
    1.0 / (1.0 + tau / seconds)
}

impl OneEuro {
    pub fn reset(&mut self, sample: Vec3, now: Option<Instant>) -> Vec3 {
        *self = Self {
            value: Some(sample),
            derivative: Vec3::ZERO,
            at: now,
        };
        sample
    }

    pub fn filter(&mut self, sample: Vec3, now: Option<Instant>) -> Vec3 {
        let (Some(value), Some(then), Some(now)) = (self.value, self.at, now) else {
            return self.reset(sample, now);
        };
        let seconds = now.saturating_duration_since(then).as_secs_f32();
        if seconds <= 0.0 {
            return value;
        }
        let derivative = (sample - value) / seconds;
        self.derivative = self
            .derivative
            .lerp(derivative, smoothing(DERIVATIVE_CUTOFF, seconds));
        let cutoff = MIN_CUTOFF + BETA * self.derivative.length();
        let filtered = value.lerp(sample, smoothing(cutoff, seconds));
        self.value = Some(filtered);
        self.at = Some(now);
        filtered
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ending {
    Trigger,
    Squeeze,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Laser {
    pub hand: Hand,
    pub aim: Option<Pose>,
    pub grip: Option<Pose>,
    pub ray: Option<Ray>,
    pub frozen: Option<Ray>,
    pub trigger: f32,
    pub pressed: bool,
    pub squeeze: f32,
    pub squeezed: bool,
    pub held: Held,
    pub ending: Ending,
    pub sent: Option<Pos2>,
    origin: OneEuro,
    direction: OneEuro,
}

impl Laser {
    pub fn new(hand: Hand) -> Self {
        Self {
            hand,
            aim: None,
            grip: None,
            ray: None,
            frozen: None,
            trigger: 0.0,
            pressed: false,
            squeeze: 0.0,
            squeezed: false,
            held: Held::default(),
            ending: Ending::Trigger,
            sent: None,
            origin: OneEuro::default(),
            direction: OneEuro::default(),
        }
    }

    pub fn aimed(&self) -> Option<Ray> {
        self.frozen.or(self.ray)
    }

    pub fn grip_point(&self) -> Option<Vec3> {
        self.grip.or(self.aim).map(|pose| pose.position)
    }

    pub fn resting(hand: Hand, head: &Head) -> Vec3 {
        let side = match hand {
            Hand::Left => -0.2,
            Hand::Right => 0.2,
        };
        head.position + head.facing() * Vec3::new(side, -0.35, -0.3)
    }

    pub fn pose(&mut self, aim: Pose, grip: Pose) {
        self.aim = Some(aim);
        self.grip = Some(grip);
    }

    pub fn smooth(&mut self, now: Option<Instant>) -> bool {
        let Some(aim) = self.aim else {
            return false;
        };
        let raw = Ray {
            origin: aim.position,
            direction: aim.rotation * Vec3::NEG_Z,
        };
        let jumped = self.ray.is_none_or(|ray| {
            ray.direction.angle_between(raw.direction) > JUMP
                || ray.origin.distance(raw.origin) > JUMP
        });
        let ray = match jumped {
            true => Ray {
                origin: self.origin.reset(raw.origin, now),
                direction: self.direction.reset(raw.direction, now),
            },
            false => Ray {
                origin: self.origin.filter(raw.origin, now),
                direction: self
                    .direction
                    .filter(raw.direction, now)
                    .normalize_or(raw.direction),
            },
        };
        let changed = self.ray != Some(ray);
        self.ray = Some(ray);
        if let Some(frozen) = self.frozen
            && frozen.direction.angle_between(ray.direction) > FREEZE
        {
            self.frozen = None;
        }
        changed
    }

    pub fn settling(&self) -> bool {
        let (Some(aim), Some(ray)) = (self.aim, self.ray) else {
            return false;
        };
        let direction = aim.rotation * Vec3::NEG_Z;
        ray.origin.distance_squared(aim.position) > SETTLED * SETTLED
            || ray.direction.distance_squared(direction) > SETTLED * SETTLED
    }

    pub fn lose(&mut self) {
        self.aim = None;
        self.grip = None;
        self.ray = None;
        self.frozen = None;
    }

    pub fn captured(&self) -> Option<WindowId> {
        self.held.captured.filter(|_| self.pressed)
    }
}

pub fn touch_id(hand: Hand) -> TouchId {
    TouchId {
        device: DEVICE,
        finger: match hand {
            Hand::Left => 0,
            Hand::Right => 1,
        },
    }
}

pub fn pose_of(pose: HandPose) -> Option<Pose> {
    let rotation = Quat::from_array(pose.orientation);
    let position = Vec3::from_array(pose.position);
    (position.is_finite() && rotation.is_finite() && rotation.length() > 1e-6).then(|| Pose {
        position,
        rotation: rotation.normalize(),
    })
}

pub fn looking(direction: Vec3) -> Quat {
    let direction = direction.normalize_or(Vec3::NEG_Z);
    let yaw = (-direction.x).atan2(-direction.z);
    let pitch = direction.y.clamp(-1.0, 1.0).asin();
    Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch)
}
