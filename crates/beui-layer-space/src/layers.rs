use beui_core::geometry::pos2;
use beui_core::window::WindowId;
use glam::{Quat, Vec3};

use crate::panel::Panel;

pub const SAMPLES_ACROSS: usize = 9;
const TOUCHING: f32 = 1e-3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Allowance {
    pub most: usize,
    pub cylinders: bool,
    pub largest: (u32, u32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PanelLayer {
    pub window: WindowId,
    pub position: Vec3,
    pub rotation: Quat,
    pub metres: glam::Vec2,
    pub bend: f32,
    pub pixels: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub version: u64,
}

pub struct Candidate {
    pub panel: Panel,
    pub pixels: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub version: u64,
}

struct Shape {
    normal: Vec3,
    back: f32,
    front: f32,
    points: Vec<Vec3>,
    margin: f32,
    centre: Vec3,
    reach: f32,
}

impl Shape {
    fn of(panel: &Panel) -> Self {
        let normal = panel.normal();
        let mut points = Vec::with_capacity(2 * SAMPLES_ACROSS);
        for index in 0..SAMPLES_ACROSS {
            let x = index as f32 / (SAMPLES_ACROSS - 1) as f32 * panel.size.x;
            points.push(panel.world(pos2(x, 0.0)));
            points.push(panel.world(pos2(x, panel.size.y)));
        }
        let back = panel.pose.position.dot(normal);
        let front = points
            .iter()
            .map(|point| point.dot(normal))
            .fold(back, f32::max);
        let margin = match panel.radius() {
            None => 0.0,
            Some(radius) => radius * (1.0 - (panel.bend / (SAMPLES_ACROSS - 1) as f32 / 2.0).cos()),
        };
        let centre = points.iter().copied().sum::<Vec3>() / points.len() as f32;
        let reach = points
            .iter()
            .map(|point| point.distance(centre))
            .fold(0.0, f32::max)
            + margin;
        Self {
            normal,
            back,
            front,
            points,
            margin,
            centre,
            reach,
        }
    }

    fn faces(&self, eye: Vec3) -> bool {
        eye.dot(self.normal) > self.back + TOUCHING
    }

    fn behind(&self, other: &Self) -> bool {
        other
            .points
            .iter()
            .all(|point| point.dot(self.normal) - other.margin <= self.back + TOUCHING)
    }

    fn before(&self, other: &Self) -> bool {
        other
            .points
            .iter()
            .all(|point| point.dot(self.normal) + other.margin >= self.front - TOUCHING)
    }

    fn apart(&self, other: &Self, eye: Vec3) -> bool {
        let cone = |shape: &Self| {
            let to = shape.centre - eye;
            let distance = to.length();
            match distance > shape.reach {
                true => Some((to / distance, (shape.reach / distance).asin())),
                false => None,
            }
        };
        match (cone(self), cone(other)) {
            (Some((a, wide_a)), Some((b, wide_b))) => a.angle_between(b) > wide_a + wide_b,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Order {
    Nearer,
    Farther,
    Apart,
}

fn order(a: &Shape, b: &Shape, eye: Vec3) -> Option<Order> {
    if a.apart(b, eye) {
        Some(Order::Apart)
    } else if a.behind(b) || b.before(a) {
        Some(Order::Nearer)
    } else if a.before(b) || b.behind(a) {
        Some(Order::Farther)
    } else {
        None
    }
}

pub fn choose(candidates: &[Candidate], eye: Vec3, allowance: &Allowance) -> Vec<PanelLayer> {
    let mut usable: Vec<(usize, Shape, f32)> = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            let (width, height) = candidate.pixels;
            (allowance.cylinders || candidate.panel.radius().is_none())
                && width <= allowance.largest.0
                && height <= allowance.largest.1
        })
        .map(|(index, candidate)| {
            let shape = Shape::of(&candidate.panel);
            let metres = candidate.panel.metres();
            let distance = shape.centre.distance(eye).max(1e-3);
            (index, shape, metres.x * metres.y / (distance * distance))
        })
        .filter(|(_, shape, _)| shape.faces(eye))
        .collect();
    usable.sort_by(|a, b| b.2.total_cmp(&a.2));
    let mut chosen: Vec<(usize, Shape)> = Vec::new();
    let mut nearer: Vec<(usize, usize)> = Vec::new();
    for (index, shape, _) in usable {
        if chosen.len() >= allowance.most {
            break;
        }
        let orders: Option<Vec<Order>> = chosen
            .iter()
            .map(|(_, other)| order(&shape, other, eye))
            .collect();
        let Some(orders) = orders else {
            continue;
        };
        let at = chosen.len();
        let mut edges = nearer.clone();
        for (other, order) in orders.into_iter().enumerate() {
            match order {
                Order::Nearer => edges.push((at, other)),
                Order::Farther => edges.push((other, at)),
                Order::Apart => {}
            }
        }
        let mut distances: Vec<f32> = chosen
            .iter()
            .map(|(_, shape)| shape.centre.distance(eye))
            .collect();
        distances.push(shape.centre.distance(eye));
        if back_to_front(&distances, &edges).is_none() {
            continue;
        }
        nearer = edges;
        chosen.push((index, shape));
    }
    let distances: Vec<f32> = chosen
        .iter()
        .map(|(_, shape)| shape.centre.distance(eye))
        .collect();
    let Some(sorted) = back_to_front(&distances, &nearer) else {
        return Vec::new();
    };
    sorted
        .into_iter()
        .map(|at| {
            let candidate = &candidates[chosen[at].0];
            let panel = &candidate.panel;
            PanelLayer {
                window: panel.window,
                position: panel.pose.position,
                rotation: panel.pose.rotation,
                metres: panel.metres(),
                bend: panel.bend,
                pixels: candidate.pixels,
                format: candidate.format,
                version: candidate.version,
            }
        })
        .collect()
}

fn back_to_front(distances: &[f32], nearer: &[(usize, usize)]) -> Option<Vec<usize>> {
    let count = distances.len();
    let mut waiting = vec![0usize; count];
    for &(near, _) in nearer {
        waiting[near] += 1;
    }
    let mut ready: Vec<usize> = (0..count).filter(|at| waiting[*at] == 0).collect();
    let mut sorted = Vec::with_capacity(count);
    while let Some(farthest) =
        (0..ready.len()).max_by(|a, b| distances[ready[*a]].total_cmp(&distances[ready[*b]]))
    {
        let at = ready.swap_remove(farthest);
        sorted.push(at);
        for &(near, far) in nearer {
            if far == at {
                waiting[near] -= 1;
                if waiting[near] == 0 {
                    ready.push(near);
                }
            }
        }
    }
    (sorted.len() == count).then_some(sorted)
}
