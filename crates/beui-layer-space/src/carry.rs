use std::time::Instant;

use beui_core::window::WindowId;
use glam::{Quat, Vec3};

use crate::contact;
use crate::head::{Head, Ray};
use crate::panel::Panel;

pub const NEAREST: f32 = 0.3;
pub const FARTHEST: f32 = 20.0;
pub const STEP_PER_LINE: f32 = 0.95;
pub const TURN_SECONDS: f32 = 0.3;
pub const PUSH_LINES: f32 = 2.0;
const PUSHED: f32 = 0.05;
const BLOCKED: f32 = 1e-4;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Front,
    Behind,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Carry {
    pub window: WindowId,
    pub distance: f32,
    rotation: Quat,
    grab: Vec3,
    direction: Vec3,
    laser: Option<Ray>,
    turn: Turn,
    sides: Vec<(WindowId, Side)>,
    asked: Option<f32>,
    at: Option<Vec3>,
    push: f32,
    ghost: Option<Panel>,
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
            sides: Vec::new(),
            asked: None,
            at: None,
            push: 0.0,
            ghost: None,
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
        self.at.unwrap_or_else(|| self.ray(head).at(self.distance))
    }

    pub fn ghost(&self) -> Option<&Panel> {
        self.ghost.as_ref()
    }

    pub fn side(&self, window: WindowId) -> Option<Side> {
        self.sides
            .iter()
            .find(|(other, _)| *other == window)
            .map(|(_, side)| *side)
    }

    pub fn settle(
        &mut self,
        head: &Head,
        panel: &mut Panel,
        others: &[Panel],
        now: Option<Instant>,
    ) {
        let eye = self.ray(head).origin;
        let grab = panel.pose.position + panel.pose.rotation * self.grab;
        let asked = grab.distance(eye).max(1e-3);
        let along = (grab - eye) / asked;
        let spans: Vec<(WindowId, f32, f32)> = others
            .iter()
            .filter(|other| other.window != self.window && contact::collides(other))
            .filter_map(|other| {
                let (lo, hi) = contact::span(panel, other, along)?;
                Some((other.window, lo, hi))
            })
            .collect();
        let was = self.at.map_or(0.0, |at| at.distance(eye) - asked);
        self.sides
            .retain(|(window, _)| spans.iter().any(|(other, ..)| other == window));
        for (window, lo, hi) in &spans {
            if self.side(*window).is_none() {
                let side = match was <= (lo + hi) / 2.0 {
                    true => Side::Front,
                    false => Side::Behind,
                };
                self.sides.push((*window, side));
            }
        }
        let (lower, upper) = self.range(&spans, asked, 0.0);
        let lines = match self.asked {
            Some(last) => (asked / last).ln() / -STEP_PER_LINE.ln(),
            None => 0.0,
        };
        self.asked = Some(asked);
        let line = -STEP_PER_LINE.ln();
        let held = match lines {
            _ if lines > 0.0 && self.push < 0.0 => lines.min(-self.push),
            _ if lines > 0.0 && self.push > 0.0 => lines,
            _ if lines > 0.0 && upper < -BLOCKED => {
                lines.min((asked / (asked + upper).max(NEAREST)).ln() / line)
            }
            _ if lines < 0.0 && self.push > 0.0 => lines.max(-self.push),
            _ if lines < 0.0 && self.push < 0.0 => lines,
            _ if lines < 0.0 && lower > BLOCKED => {
                lines.max(-((asked + lower) / asked).ln() / line)
            }
            _ => 0.0,
        };
        self.push += held;
        let kept = asked * STEP_PER_LINE.powf(held);
        let shift = asked - kept;
        if held != 0.0 {
            self.distance *= kept / asked;
            self.asked = Some(kept);
        }
        let (mut lower, mut upper) = self.range(&spans, kept, shift);
        if (self.push > 0.0 && upper > BLOCKED) || (self.push < 0.0 && lower < -BLOCKED) {
            self.push = 0.0;
        }
        let mut origin = kept;
        if self.push.abs() >= PUSH_LINES {
            let through = kept / STEP_PER_LINE.powf(self.push) - kept;
            let mut passed = false;
            for (window, lo, hi) in &spans {
                let (lo, hi) = (lo + shift, hi + shift);
                let side = self.side(*window);
                let flip = match side {
                    Some(Side::Front) => self.push > 0.0 && lo < through && through >= hi,
                    Some(Side::Behind) => self.push < 0.0 && hi > through && through <= lo,
                    None => false,
                };
                if flip {
                    passed = true;
                    for (other, side) in &mut self.sides {
                        if other == window {
                            *side = match side {
                                Side::Front => Side::Behind,
                                Side::Behind => Side::Front,
                            };
                        }
                    }
                }
            }
            if passed {
                origin = kept + through;
                self.distance *= origin / kept;
                self.asked = Some(origin);
                self.push = 0.0;
                (lower, upper) = self.range(&spans, origin, shift - through);
            }
        }
        let offset = shift - (origin - kept);
        let moved = match lower <= upper {
            true => 0.0_f32.clamp(lower, upper),
            false => self.free(&spans, origin, offset, was + asked - origin),
        };
        let pushed = (self.push.abs() > PUSHED).then(|| {
            let mut ghost = *panel;
            let to = origin / STEP_PER_LINE.powf(self.push);
            ghost.pose.position += along * (to - asked);
            ghost
        });
        self.ghost = pushed;
        let at = eye + along * (origin + moved);
        self.place_at(head, panel, at, now);
        self.at = Some(at);
    }

    fn range(&self, spans: &[(WindowId, f32, f32)], origin: f32, shift: f32) -> (f32, f32) {
        let mut lower = NEAREST - origin;
        let mut upper = FARTHEST - origin;
        for (window, lo, hi) in spans {
            match self.side(*window) {
                Some(Side::Front) => upper = upper.min(lo + shift),
                Some(Side::Behind) => lower = lower.max(hi + shift),
                None => {}
            }
        }
        (lower, upper)
    }

    fn free(&mut self, spans: &[(WindowId, f32, f32)], origin: f32, shift: f32, was: f32) -> f32 {
        let (lower, upper) = (NEAREST - origin, FARTHEST - origin);
        let inside = |at: f32| {
            spans
                .iter()
                .any(|(_, lo, hi)| lo + shift < at && at < hi + shift)
        };
        let moved = spans
            .iter()
            .flat_map(|(_, lo, hi)| [lo + shift, hi + shift])
            .chain([0.0, was])
            .map(|at| at.clamp(lower, upper))
            .filter(|at| !inside(*at))
            .min_by(|a, b| {
                (a - was)
                    .abs()
                    .total_cmp(&(b - was).abs())
                    .then(a.total_cmp(b))
            })
            .unwrap_or(0.0_f32.clamp(lower, upper));
        for (window, lo, _) in spans {
            let side = match moved <= lo + shift {
                true => Side::Front,
                false => Side::Behind,
            };
            for (other, kept) in &mut self.sides {
                if other == window {
                    *kept = side;
                }
            }
        }
        moved
    }

    pub fn place(&self, head: &Head, panel: &mut Panel, now: Option<Instant>) {
        self.place_at(head, panel, self.ray(head).at(self.distance), now);
    }

    fn place_at(&self, head: &Head, panel: &mut Panel, grab: Vec3, now: Option<Instant>) {
        let carried = head.rotation() * self.rotation;
        panel.pose.rotation = facing(head.position, grab, self.grab, head.up())
            .map_or(carried, |target| self.turn.toward(carried, target, now));
        panel.pose.position = grab - panel.pose.rotation * self.grab;
    }
}
