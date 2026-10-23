use beui_core::geometry::{Pos2, Vec2, pos2, vec2};
use beui_core::window::{HostPlacement, WindowId};
use glam::{Mat4, Quat, Vec3};

use crate::head::Ray;

pub const POINTS_PER_METRE: f32 = 1000.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Pose {
    pub position: Vec3,
    pub rotation: Quat,
}

impl Pose {
    pub fn placement(&self) -> HostPlacement {
        let Vec3 { x, y, z } = self.position;
        let [qx, qy, qz, qw] = self.rotation.to_array();
        HostPlacement(format!("space {x} {y} {z} {qx} {qy} {qz} {qw}"))
    }

    pub fn from_placement(placement: &HostPlacement) -> Option<Self> {
        let mut words = placement.0.split(' ');
        if words.next()? != "space" {
            return None;
        }
        let numbers: Vec<f32> = words.map(str::parse).collect::<Result<_, _>>().ok()?;
        let [x, y, z, qx, qy, qz, qw] = numbers.try_into().ok()?;
        let rotation = Quat::from_xyzw(qx, qy, qz, qw);
        Some(Self {
            position: Vec3::new(x, y, z),
            rotation: match rotation.is_normalized() {
                true => rotation,
                false => rotation.normalize(),
            },
        })
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Hit {
    pub window: WindowId,
    pub distance: f32,
    pub pos: Pos2,
    pub point: Vec3,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Panel {
    pub window: WindowId,
    pub pose: Pose,
    pub size: Vec2,
}

impl Panel {
    pub fn metres(&self) -> glam::Vec2 {
        glam::Vec2::new(self.size.x, self.size.y) / POINTS_PER_METRE
    }

    pub fn normal(&self) -> Vec3 {
        self.pose.rotation * Vec3::Z
    }

    pub fn offset(&self, pos: Pos2) -> Vec3 {
        Vec3::new(
            pos.x / POINTS_PER_METRE - self.metres().x / 2.0,
            self.metres().y / 2.0 - pos.y / POINTS_PER_METRE,
            0.0,
        )
    }

    pub fn world(&self, pos: Pos2) -> Vec3 {
        self.pose.position + self.pose.rotation * self.offset(pos)
    }

    pub fn model(&self) -> Mat4 {
        let metres = self.metres();
        Mat4::from_scale_rotation_translation(
            Vec3::new(metres.x, metres.y, 1.0),
            self.pose.rotation,
            self.pose.position,
        )
    }

    pub fn intersect(&self, ray: Ray) -> Option<Hit> {
        let normal = self.normal();
        let facing = ray.direction.dot(normal);
        if facing >= -1e-6 {
            return None;
        }
        let distance = (self.pose.position - ray.origin).dot(normal) / facing;
        if distance <= 0.0 {
            return None;
        }
        let point = ray.at(distance);
        let local = self.pose.rotation.inverse() * (point - self.pose.position);
        let metres = self.metres();
        let pos = pos2(
            (local.x + metres.x / 2.0) * POINTS_PER_METRE,
            (metres.y / 2.0 - local.y) * POINTS_PER_METRE,
        );
        Some(Hit {
            window: self.window,
            distance,
            pos,
            point,
        })
    }

    pub fn hit(&self, ray: Ray) -> Option<Hit> {
        self.intersect(ray).filter(|hit| {
            (0.0..=self.size.x).contains(&hit.pos.x) && (0.0..=self.size.y).contains(&hit.pos.y)
        })
    }

    pub fn face(&mut self, eye: Vec3) {
        let toward = eye - self.pose.position;
        if vec2(toward.x, toward.z).length() < 1e-4 {
            return;
        }
        self.pose.rotation = Quat::from_rotation_y(toward.x.atan2(toward.z));
    }
}

pub fn pick(panels: &[Panel], ray: Ray) -> Option<Hit> {
    panels
        .iter()
        .filter_map(|panel| panel.hit(ray))
        .min_by(|a, b| a.distance.total_cmp(&b.distance))
}
