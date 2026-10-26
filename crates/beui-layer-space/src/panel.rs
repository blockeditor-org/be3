use beui_core::geometry::{Pos2, Vec2, pos2};
use beui_core::window::{HostPlacement, WindowId};
use glam::{Mat4, Quat, Vec3};

use crate::head::Ray;

pub const POINTS_PER_METRE: f32 = 1000.0;
pub const MOST_BEND: f32 = std::f32::consts::PI;
const FLAT: f32 = 1e-4;

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hit {
    pub window: WindowId,
    pub distance: f32,
    pub pos: Pos2,
    pub point: Vec3,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Panel {
    pub window: WindowId,
    pub pose: Pose,
    pub size: Vec2,
    pub metres_per_point: f32,
    pub bend: f32,
    pub chrome: crate::chrome::Fit,
}

impl Panel {
    pub fn new(window: WindowId, pose: Pose, size: Vec2) -> Self {
        Self {
            window,
            pose,
            size,
            metres_per_point: 1.0 / POINTS_PER_METRE,
            bend: 0.0,
            chrome: crate::chrome::Fit::default(),
        }
    }

    pub fn placement(&self) -> HostPlacement {
        let Vec3 { x, y, z } = self.pose.position;
        let [qx, qy, qz, qw] = self.pose.rotation.to_array();
        let (scale, bend) = (self.metres_per_point, self.bend);
        HostPlacement(format!(
            "space {x} {y} {z} {qx} {qy} {qz} {qw} {scale} {bend}"
        ))
    }

    pub fn placed(&mut self, placement: &HostPlacement) -> bool {
        let Some(numbers) = placement.0.strip_prefix("space ").and_then(|rest| {
            rest.split(' ')
                .map(str::parse::<f32>)
                .collect::<Result<Vec<f32>, _>>()
                .ok()
        }) else {
            return false;
        };
        let Ok([x, y, z, qx, qy, qz, qw, scale, bend]) = <[f32; 9]>::try_from(numbers) else {
            return false;
        };
        let rotation = Quat::from_xyzw(qx, qy, qz, qw);
        if ![x, y, z, scale, bend].iter().all(|value| value.is_finite())
            || !rotation.is_finite()
            || rotation.length() < 1e-6
            || scale <= 0.0
        {
            return false;
        }
        self.pose = Pose {
            position: Vec3::new(x, y, z),
            rotation: match rotation.is_normalized() {
                true => rotation,
                false => rotation.normalize(),
            },
        };
        self.metres_per_point = scale;
        self.bend = bend.clamp(0.0, MOST_BEND);
        true
    }

    pub fn metres(&self) -> glam::Vec2 {
        glam::Vec2::new(self.size.x, self.size.y) * self.metres_per_point
    }

    pub fn normal(&self) -> Vec3 {
        self.pose.rotation * Vec3::Z
    }

    pub fn radius(&self) -> Option<f32> {
        (self.bend > FLAT).then(|| self.metres().x / self.bend)
    }

    fn along(&self, pos: Pos2) -> (f32, f32) {
        let metres = self.metres();
        (
            pos.x * self.metres_per_point - metres.x / 2.0,
            metres.y / 2.0 - pos.y * self.metres_per_point,
        )
    }

    pub fn offset(&self, pos: Pos2) -> Vec3 {
        let (across, up) = self.along(pos);
        bent(self.metres().x, self.bend, across, up, 0.0)
    }

    pub fn world(&self, pos: Pos2) -> Vec3 {
        self.pose.position + self.pose.rotation * self.offset(pos)
    }

    pub fn surface_turn(&self, pos: Pos2) -> Quat {
        match self.radius() {
            None => Quat::IDENTITY,
            Some(radius) => Quat::from_rotation_y(-self.along(pos).0 / radius),
        }
    }

    pub fn frame_at(&self, pos: Pos2) -> (Vec3, Quat) {
        (self.world(pos), self.pose.rotation * self.surface_turn(pos))
    }

    pub fn anchor(&mut self, pos: Pos2, (point, rotation): (Vec3, Quat)) {
        self.pose.rotation = (rotation * self.surface_turn(pos).inverse()).normalize();
        self.pose.position = point - self.pose.rotation * self.offset(pos);
    }

    pub fn pos_of(&self, local: Vec3) -> Pos2 {
        let across = match self.radius() {
            None => local.x,
            Some(radius) => local.x.atan2(radius - local.z) * radius,
        };
        let metres = self.metres();
        pos2(
            (across + metres.x / 2.0) / self.metres_per_point,
            (metres.y / 2.0 - local.y) / self.metres_per_point,
        )
    }

    pub fn model(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.pose.rotation, self.pose.position)
    }

    pub fn intersect(&self, ray: Ray) -> Option<Hit> {
        let inverse = self.pose.rotation.inverse();
        let origin = inverse * (ray.origin - self.pose.position);
        let direction = inverse * ray.direction;
        let distance = match self.radius() {
            None => {
                if direction.z >= -1e-6 {
                    return None;
                }
                -origin.z / direction.z
            }
            Some(radius) => {
                let (x, z) = (origin.x, origin.z - radius);
                let a = direction.x * direction.x + direction.z * direction.z;
                if a < 1e-12 {
                    return None;
                }
                let b = 2.0 * (x * direction.x + z * direction.z);
                let c = x * x + z * z - radius * radius;
                let discriminant = b * b - 4.0 * a * c;
                if discriminant < 0.0 {
                    return None;
                }
                (-b + discriminant.sqrt()) / (2.0 * a)
            }
        };
        if distance <= 0.0 {
            return None;
        }
        Some(Hit {
            window: self.window,
            distance,
            pos: self.pos_of(origin + direction * distance),
            point: ray.at(distance),
        })
    }

    pub fn contains(&self, pos: Pos2) -> bool {
        (0.0..=self.size.x).contains(&pos.x) && (0.0..=self.size.y).contains(&pos.y)
    }

    pub fn hit(&self, ray: Ray) -> Option<Hit> {
        self.intersect(ray).filter(|hit| self.contains(hit.pos))
    }

    pub fn face(&mut self, eye: Vec3, up: Vec3) {
        if let Some(rotation) = crate::carry::facing(eye, self.pose.position, Vec3::ZERO, up) {
            self.pose.rotation = rotation;
        }
    }
}

pub fn bent(width: f32, bend: f32, across: f32, up: f32, lift: f32) -> Vec3 {
    if bend <= FLAT {
        return Vec3::new(across, up, lift);
    }
    let radius = width / bend;
    let angle = across / radius;
    let normal = Vec3::new(-angle.sin(), 0.0, angle.cos());
    Vec3::new(radius * angle.sin(), up, radius * (1.0 - angle.cos())) + normal * lift
}

pub fn pick(panels: &[Panel], ray: Ray) -> Option<Hit> {
    panels
        .iter()
        .filter_map(|panel| panel.hit(ray))
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
}
