use beui_core::geometry::{Pos2, Rect, pos2};
use glam::Vec3;

use crate::chrome;
use crate::head::Ray;
use crate::panel::Panel;

pub const CLEARANCE: f32 = 0.012;
const RAMP: f32 = 1.0;
const SAMPLES: usize = 7;
const REACH: f32 = 50.0;

pub fn collides(panel: &Panel) -> bool {
    panel.framed()
}

pub fn span(moving: &Panel, other: &Panel, along: Vec3) -> Option<(f32, f32)> {
    let along = along.normalize_or(Vec3::NEG_Z);
    let line = |point: Vec3| Ray {
        origin: point - along * REACH,
        direction: along,
    };
    let (mine, theirs) = (extent(moving), extent(other));
    let forward = samples(mine).filter_map(|pos| {
        let hit = other.intersect(line(moving.world(pos)))?;
        Some((hit.distance - REACH, outside(other, theirs, hit.pos)))
    });
    let back = samples(theirs).filter_map(|pos| {
        let hit = moving.intersect(line(other.world(pos)))?;
        Some((REACH - hit.distance, outside(moving, mine, hit.pos)))
    });
    forward
        .chain(back)
        .filter_map(|(at, outside)| {
            let clearance = CLEARANCE - RAMP * outside;
            (clearance > 0.0).then_some((at - clearance, at + clearance))
        })
        .reduce(|(lo, hi), (low, high)| (lo.min(low), hi.max(high)))
}

pub fn touching(moving: &Panel, other: &Panel, eye: Vec3) -> bool {
    span(moving, other, moving.pose.position - eye).is_some_and(|(lo, hi)| lo < 0.0 && hi > 0.0)
}

pub fn newly_touching(before: &Panel, after: &Panel, others: &[Panel], eye: Vec3) -> bool {
    others
        .iter()
        .filter(|other| other.window != after.window && collides(other))
        .any(|other| touching(after, other, eye) && !touching(before, other, eye))
}

fn extent(panel: &Panel) -> Rect {
    let whole = Rect::from_min_size(Pos2::ZERO, panel.size);
    if !panel.framed() {
        return whole;
    }
    let bounds = chrome::bounds(panel);
    let points = |at: Pos2| pos2(at.x / panel.metres_per_point, at.y / panel.metres_per_point);
    Rect::from_min_max(points(bounds.min), points(bounds.max)).union(whole)
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
