use std::time::Instant;

use beui_core::geometry::{Pos2, Rect, pos2};
use beui_core::window::WindowId;
use glam::{Quat, Vec3};

use crate::chrome;
use crate::head::{Head, Ray};
use crate::panel::Panel;

pub const NEAREST: f32 = 0.3;
pub const FARTHEST: f32 = 20.0;
pub const STEP_PER_LINE: f32 = 0.9;
pub const TURN_SECONDS: f32 = 0.3;
pub const IN_FRONT: f32 = 0.015;
pub const RAMP: f32 = 1.0;
const SAMPLES: usize = 5;
const STEPS: usize = 12;
const SETTLED: f32 = 1e-4;

#[derive(Clone, PartialEq, Debug)]
pub struct Carry {
    pub window: WindowId,
    pub distance: f32,
    pub at: f32,
    pub behind: Vec<WindowId>,
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

pub fn facing(eye: Vec3, grab: Vec3, offset: Vec3, roll: f32) -> Option<Quat> {
    let rolled = Quat::from_rotation_z(roll);
    let offset = rolled * offset;
    let toward = eye - grab;
    let level = toward.length_squared() - offset.x * offset.x - offset.y * offset.y;
    if level < 1e-8 {
        return None;
    }
    let (y, z) = (-offset.y, level.sqrt());
    let reach = y.hypot(z);
    let tilt = (toward.y / reach).clamp(-1.0, 1.0).acos() - z.atan2(y);
    let ahead = y * tilt.sin() + z * tilt.cos();
    if toward.x.hypot(toward.z) < 1e-4 {
        return None;
    }
    let turn = toward.x.atan2(toward.z) - (-offset.x).atan2(ahead);
    Some(Quat::from_rotation_y(turn) * Quat::from_rotation_x(tilt) * rolled)
}

impl Carry {
    pub fn begin(panel: &Panel, head: &Head, grab: Vec3, now: Option<Instant>) -> Self {
        let to_grab = grab - head.position;
        let inverse = head.rotation().inverse();
        let distance = to_grab.length().max(NEAREST);
        Self {
            window: panel.window,
            distance,
            at: distance,
            behind: Vec::new(),
            rotation: inverse * panel.pose.rotation,
            grab: panel.pose.rotation.inverse() * (grab - panel.pose.position),
            direction: (inverse * to_grab).normalize_or(Vec3::NEG_Z),
            turn: Turn::begin(now),
        }
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

    pub fn scroll(
        &mut self,
        lines: f32,
        head: &Head,
        panel: &Panel,
        others: &[Panel],
        now: Option<Instant>,
    ) {
        let mut distance = (self.at / STEP_PER_LINE.powf(lines)).clamp(NEAREST, FARTHEST);
        let eye = head.position;
        if lines > 0.0 {
            for _ in 0..others.len() {
                let placed = self.placed(head, panel, distance, now);
                let Some(through) = self
                    .blocking(others)
                    .find(|other| gaps(&placed, other, eye).any(|gap| gap.pull() > 0.0))
                    .copied()
                else {
                    break;
                };
                self.behind.push(through.window);
                for _ in 0..STEPS {
                    let placed = self.placed(head, panel, distance, now);
                    let short = IN_FRONT - clearance(&placed, &through, eye);
                    if short <= SETTLED || distance >= FARTHEST {
                        break;
                    }
                    distance = (distance + short).min(FARTHEST);
                }
            }
        } else {
            let placed = self.placed(head, panel, distance, now);
            self.behind.retain(|window| {
                others
                    .iter()
                    .find(|other| other.window == *window)
                    .is_some_and(|other| clearance(&placed, other, eye) >= IN_FRONT)
            });
        }
        self.distance = distance;
    }

    pub fn grabbed_at(&self, head: &Head) -> Vec3 {
        head.position + head.rotation() * self.direction * self.at
    }

    pub fn place(
        &mut self,
        head: &Head,
        panel: &mut Panel,
        others: &[Panel],
        now: Option<Instant>,
    ) {
        self.at = self.reach(head, panel, others, now);
        *panel = self.placed(head, panel, self.at, now);
    }

    fn blocking<'a>(&'a self, others: &'a [Panel]) -> impl Iterator<Item = &'a Panel> {
        others
            .iter()
            .filter(|other| other.window != self.window && !self.behind.contains(&other.window))
    }

    fn reach(&self, head: &Head, panel: &Panel, others: &[Panel], now: Option<Instant>) -> f32 {
        let pull = |distance: f32| {
            let placed = self.placed(head, panel, distance, now);
            self.blocking(others)
                .flat_map(|other| gaps(&placed, other, head.position))
                .map(Gap::pull)
                .fold(f32::NEG_INFINITY, f32::max)
        };
        let (mut far, mut short) = (self.distance, pull(self.distance));
        for _ in 0..STEPS {
            if short <= SETTLED || far <= NEAREST {
                return far;
            }
            let near = (far - short).max(NEAREST);
            let left = pull(near);
            if left > SETTLED {
                (far, short) = (near, left);
                continue;
            }
            let mut clear = near;
            for _ in 0..STEPS {
                let middle = (clear + far) / 2.0;
                match pull(middle) > 0.0 {
                    true => far = middle,
                    false => clear = middle,
                }
            }
            return clear;
        }
        far
    }

    fn placed(&self, head: &Head, panel: &Panel, distance: f32, now: Option<Instant>) -> Panel {
        let rotation = head.rotation();
        let grab = head.position + rotation * self.direction * distance;
        let carried = rotation * self.rotation;
        let mut panel = *panel;
        panel.pose.rotation = facing(head.position, grab, self.grab, head.roll)
            .map_or(carried, |target| self.turn.toward(carried, target, now));
        panel.pose.position = grab - panel.pose.rotation * self.grab;
        panel
    }
}

#[derive(Clone, Copy, Debug)]
struct Gap {
    behind: f32,
    outside: f32,
}

impl Gap {
    fn pull(self) -> f32 {
        self.behind + IN_FRONT - RAMP * self.outside
    }
}

fn clearance(carried: &Panel, other: &Panel, eye: Vec3) -> f32 {
    gaps(carried, other, eye)
        .filter(|gap| gap.outside <= 0.0)
        .map(|gap| gap.behind)
        .fold(f32::INFINITY, f32::min)
}

fn extent(panel: &Panel) -> Rect {
    let bounds = chrome::bounds(panel);
    let points = |at: Pos2| pos2(at.x / panel.metres_per_point, at.y / panel.metres_per_point);
    Rect::from_min_max(points(bounds.min), points(bounds.max))
        .union(Rect::from_min_size(Pos2::ZERO, panel.size))
}

fn samples(rect: Rect) -> impl Iterator<Item = Pos2> {
    let step = |index: usize| index as f32 / (SAMPLES - 1) as f32;
    (0..SAMPLES).flat_map(move |row| {
        (0..SAMPLES).map(move |column| {
            pos2(
                rect.min.x + rect.width() * step(column),
                rect.min.y + rect.height() * step(row),
            )
        })
    })
}

fn outside(panel: &Panel, rect: Rect, pos: Pos2) -> f32 {
    let x = (rect.min.x - pos.x).max(pos.x - rect.max.x).max(0.0);
    let y = (rect.min.y - pos.y).max(pos.y - rect.max.y).max(0.0);
    x.hypot(y) * panel.metres_per_point
}

fn gaps<'a>(carried: &'a Panel, other: &'a Panel, eye: Vec3) -> impl Iterator<Item = Gap> + 'a {
    let toward = move |point: Vec3| Ray {
        origin: eye,
        direction: (point - eye).normalize_or(Vec3::NEG_Z),
    };
    let reach = extent(carried);
    let whole = Rect::from_min_size(Pos2::ZERO, other.size);
    let forward = samples(reach).filter_map(move |pos| {
        let point = carried.world(pos);
        let hit = other.intersect(toward(point))?;
        Some(Gap {
            behind: point.distance(eye) - hit.distance,
            outside: outside(other, whole, hit.pos),
        })
    });
    let back = samples(whole).filter_map(move |pos| {
        let point = other.world(pos);
        let hit = carried.intersect(toward(point))?;
        Some(Gap {
            behind: hit.distance - point.distance(eye),
            outside: outside(carried, reach, hit.pos),
        })
    });
    forward.chain(back)
}
