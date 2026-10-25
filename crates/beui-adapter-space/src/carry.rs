use std::time::Instant;

use beui_core::window::WindowId;
use glam::{Quat, Vec3};

use crate::head::Head;
use crate::panel::Panel;

pub const NEAREST: f32 = 0.3;
pub const FARTHEST: f32 = 20.0;
pub const STEP_PER_LINE: f32 = 0.9;
pub const TURN_SECONDS: f32 = 0.3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Carry {
    pub window: WindowId,
    pub distance: f32,
    lifted_within: f32,
    rotation: Quat,
    grab: Vec3,
    direction: Vec3,
    turn: Turn,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Turn {
    began: Option<Instant>,
}

impl Turn {
    pub fn begin(now: Option<Instant>) -> Self {
        Self { began: now }
    }

    pub fn progress(&self, now: Option<Instant>) -> f32 {
        let (Some(began), Some(now)) = (self.began, now) else {
            return 0.0;
        };
        let t = (now.saturating_duration_since(began).as_secs_f32() / TURN_SECONDS).min(1.0);
        t * t * (3.0 - 2.0 * t)
    }

    pub fn turning(&self, now: Option<Instant>) -> bool {
        self.progress(now) < 1.0
    }

    pub fn toward(&self, from: Quat, to: Quat, now: Option<Instant>) -> Quat {
        from.slerp(to, self.progress(now))
    }
}

pub fn facing(eye: Vec3, grab: Vec3, offset: Vec3) -> Option<Quat> {
    let toward = eye - grab;
    let reach = toward.x.hypot(toward.z);
    if reach < 1e-4 {
        return None;
    }
    let aside = (offset.x / reach).clamp(-1.0, 1.0).asin();
    Some(Quat::from_rotation_y(toward.x.atan2(toward.z) + aside))
}

impl Carry {
    pub fn begin(panel: &Panel, head: &Head, grab: Vec3, now: Option<Instant>) -> Self {
        let to_grab = grab - head.position;
        let inverse = head.rotation().inverse();
        let distance = to_grab.length().max(NEAREST);
        Self {
            window: panel.window,
            distance,
            lifted_within: distance,
            rotation: inverse * panel.pose.rotation,
            grab: panel.pose.rotation.inverse() * (grab - panel.pose.position),
            direction: (inverse * to_grab).normalize_or(Vec3::NEG_Z),
            turn: Turn::begin(now),
        }
    }

    pub fn lifted(&self) -> bool {
        self.distance <= self.lifted_within
    }

    pub fn turning(&self, now: Option<Instant>) -> bool {
        self.turn.turning(now)
    }

    pub fn aim(&mut self, head: &Head, direction: Vec3) {
        let direction = direction.normalize_or(self.direction);
        let turned = head.rotation();
        let yaw = |toward: Vec3| {
            let toward = turned * toward;
            toward.x.atan2(toward.z)
        };
        let swing = Quat::from_rotation_y(yaw(direction) - yaw(self.direction));
        self.rotation = turned.inverse() * swing * turned * self.rotation;
        self.direction = direction;
    }

    pub fn scroll(&mut self, lines: f32) {
        self.distance = (self.distance / STEP_PER_LINE.powf(lines)).clamp(NEAREST, FARTHEST);
    }

    pub fn grabbed_at(&self, head: &Head) -> Vec3 {
        head.position + head.rotation() * self.direction * self.distance
    }

    pub fn place(&self, head: &Head, panel: &mut Panel, now: Option<Instant>) {
        let rotation = head.rotation();
        let grab = self.grabbed_at(head);
        let carried = rotation * self.rotation;
        panel.pose.rotation = facing(head.position, grab, self.grab)
            .map_or(carried, |target| self.turn.toward(carried, target, now));
        panel.pose.position = grab - panel.pose.rotation * self.grab;
    }
}
