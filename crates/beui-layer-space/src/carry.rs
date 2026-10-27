use std::time::Instant;

use beui_core::window::WindowId;
use glam::{Quat, Vec3};

use crate::head::{Head, Ray};
use crate::panel::Panel;

pub const NEAREST: f32 = 0.3;
pub const FARTHEST: f32 = 20.0;
pub const STEP_PER_LINE: f32 = 0.95;
pub const TURN_SECONDS: f32 = 0.3;

#[derive(Clone, PartialEq, Debug)]
pub struct Carry {
    pub window: WindowId,
    pub distance: f32,
    rotation: Quat,
    grab: Vec3,
    direction: Vec3,
    laser: Option<Ray>,
    turn: Turn,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Turn {
    began: Option<Instant>,
    finished: bool,
}

impl Turn {
    pub fn begin(now: Option<Instant>) -> Self {
        Self {
            began: now,
            finished: false,
        }
    }

    pub fn progress(&self, now: Option<Instant>) -> f32 {
        if self.finished {
            return 1.0;
        }
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

pub fn facing(eye: Vec3, grab: Vec3, offset: Vec3, up: Vec3) -> Option<Quat> {
    let toward = eye - grab;
    let level = toward.length_squared() - offset.x * offset.x - offset.y * offset.y;
    if level < 1e-8 {
        return None;
    }
    let from = Vec3::new(-offset.x, -offset.y, level.sqrt()).normalize();
    let to = toward.normalize();
    let base = Quat::from_rotation_arc(from, to);
    let across = base * Vec3::X;
    let along = to * across.dot(to);
    let around = across - along;
    let (a, b, c) = (around.dot(up), to.cross(around).dot(up), along.dot(up));
    let reach = a.hypot(b);
    if reach < 1e-6 {
        return None;
    }
    let (middle, spread) = (b.atan2(a), (-c / reach).clamp(-1.0, 1.0).acos());
    [middle + spread, middle - spread]
        .map(|angle| (Quat::from_axis_angle(to, angle) * base).normalize())
        .into_iter()
        .max_by(|x, y| (*x * Vec3::Y).dot(up).total_cmp(&(*y * Vec3::Y).dot(up)))
}

impl Carry {
    pub fn begin(panel: &Panel, head: &Head, grab: Vec3, now: Option<Instant>) -> Self {
        let to_grab = grab - head.position;
        let inverse = head.rotation().inverse();
        let distance = to_grab.length().max(NEAREST);
        Self {
            window: panel.window,
            distance,
            rotation: inverse * panel.pose.rotation,
            grab: panel.pose.rotation.inverse() * (grab - panel.pose.position),
            direction: (inverse * to_grab).normalize_or(Vec3::NEG_Z),
            laser: None,
            turn: Turn::begin(now),
        }
    }

    pub fn along(panel: &Panel, head: &Head, laser: Ray, grab: Vec3, now: Option<Instant>) -> Self {
        let distance = (grab - laser.origin).length().max(NEAREST);
        Self {
            distance,
            laser: Some(laser),
            ..Self::begin(panel, head, grab, now)
        }
    }

    pub fn follow(&mut self, laser: Ray) {
        if self.laser.is_some() {
            self.laser = Some(laser);
        }
    }

    fn ray(&self, head: &Head) -> Ray {
        self.laser.unwrap_or(Ray {
            origin: head.position,
            direction: head.rotation() * self.direction,
        })
    }

    pub fn turning(&self, now: Option<Instant>) -> bool {
        self.turn.turning(now)
    }

    pub fn finish(&mut self) {
        self.turn.finished = true;
    }

    pub fn aim(&mut self, direction: Vec3) {
        let direction = direction.normalize_or(self.direction);
        let yaw = |toward: Vec3| toward.x.atan2(toward.z);
        let swing = Quat::from_rotation_y(yaw(direction) - yaw(self.direction));
        self.rotation = (swing * self.rotation).normalize();
        self.direction = direction;
    }

    pub fn scroll(&mut self, lines: f32) {
        self.distance = (self.distance / STEP_PER_LINE.powf(lines)).clamp(NEAREST, FARTHEST);
    }

    pub fn grabbed_at(&self, head: &Head) -> Vec3 {
        self.ray(head).at(self.distance)
    }

    pub fn place(&self, head: &Head, panel: &mut Panel, now: Option<Instant>) {
        let grab = self.ray(head).at(self.distance);
        let carried = head.rotation() * self.rotation;
        panel.pose.rotation = facing(head.position, grab, self.grab, head.up())
            .map_or(carried, |target| self.turn.toward(carried, target, now));
        panel.pose.position = grab - panel.pose.rotation * self.grab;
    }
}
