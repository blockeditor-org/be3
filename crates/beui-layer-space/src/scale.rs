use std::time::Duration;

use beui_core::geometry::{Pos2, Vec2, pos2};
use glam::Vec4;

use crate::head::View;
use crate::panel::Panel;

pub const SUPERSAMPLE: f32 = 1.25;
pub const STEPS_PER_DOUBLING: f32 = 4.0;
pub const KEPT_STEPS: f32 = 2.0;
pub const SLACK_STEPS: f32 = 0.1;
pub const LEAST_SCALE: f32 = 0.5;
pub const MOST_SCALE: f32 = 4.0;
pub const LARGEST_TEXTURE: f32 = 4096.0;
const OUTLINE_STEPS: usize = 8;
pub const SETTLE: Duration = Duration::from_millis(250);

pub fn wanted(panel: &Panel, views: &[View]) -> Option<f32> {
    views
        .iter()
        .filter_map(|view| wanted_by(panel, view))
        .reduce(f32::max)
}

fn wanted_by(panel: &Panel, view: &View) -> Option<f32> {
    if view.size.x < 1.0 || view.size.y < 1.0 {
        return None;
    }
    let view_projection = view.view_projection();
    let along = |step: usize| panel.size.x * step as f32 / OUTLINE_STEPS as f32;
    let outline: Vec<Vec4> = (0..=OUTLINE_STEPS)
        .map(|step| pos2(along(step), 0.0))
        .chain(
            (0..=OUTLINE_STEPS)
                .rev()
                .map(|step| pos2(along(step), panel.size.y)),
        )
        .map(|at: Pos2| view_projection * panel.world(at).extend(1.0))
        .collect();
    let nearest = clipped(outline)
        .iter()
        .map(|corner| corner.w)
        .reduce(f32::min)?;
    let focal = view.fov.pixels_per_tangent(view.size);
    Some(SUPERSAMPLE * focal / nearest.max(f32::EPSILON) * panel.metres_per_point)
}

fn clipped(mut polygon: Vec<Vec4>) -> Vec<Vec4> {
    let planes: [fn(Vec4) -> f32; 5] = [
        |at| at.w + at.x,
        |at| at.w - at.x,
        |at| at.w + at.y,
        |at| at.w - at.y,
        |at| at.z,
    ];
    for plane in planes {
        let mut kept = Vec::with_capacity(polygon.len() + 1);
        for (index, &at) in polygon.iter().enumerate() {
            let next = polygon[(index + 1) % polygon.len()];
            let (here, there) = (plane(at), plane(next));
            if here >= 0.0 {
                kept.push(at);
            }
            if (here >= 0.0) != (there >= 0.0) {
                kept.push(at.lerp(next, here / (here - there)));
            }
        }
        polygon = kept;
        if polygon.is_empty() {
            break;
        }
    }
    polygon
}

fn step(scale: f32) -> f32 {
    scale.max(f32::MIN_POSITIVE).log2() * STEPS_PER_DOUBLING
}

fn of_step(step: f32) -> f32 {
    (step / STEPS_PER_DOUBLING).exp2()
}

pub fn chosen(held: Option<f32>, wanted: Option<f32>, size: Vec2, platform: f32) -> f32 {
    let largest = of_step(step(LARGEST_TEXTURE / size.x.max(size.y).max(1.0)).floor());
    let most = MOST_SCALE.min(largest);
    let scale = match (held, wanted) {
        (Some(held), None) => held,
        (Some(held), Some(wanted))
            if wanted <= held && step(wanted) > step(held) - KEPT_STEPS - 1e-3 =>
        {
            held
        }
        (_, Some(wanted)) => of_step((step(wanted) - SLACK_STEPS).ceil()),
        (None, None) => platform,
    };
    scale.clamp(LEAST_SCALE.min(most), most)
}
