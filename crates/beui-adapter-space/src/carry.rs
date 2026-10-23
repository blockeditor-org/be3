use beui_core::window::WindowId;
use glam::{Quat, Vec3};

use crate::head::Head;
use crate::panel::Panel;

pub const NEAREST: f32 = 0.3;
pub const FARTHEST: f32 = 20.0;
pub const STEP_PER_LINE: f32 = 0.9;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Carry {
    pub window: WindowId,
    pub distance: f32,
    rotation: Quat,
    grab: Vec3,
    direction: Vec3,
}

impl Carry {
    pub fn begin(panel: &Panel, head: &Head, grab: Vec3) -> Self {
        let to_grab = grab - head.position;
        let inverse = head.rotation().inverse();
        Self {
            window: panel.window,
            distance: to_grab.length().max(NEAREST),
            rotation: inverse * panel.pose.rotation,
            grab: panel.pose.rotation.inverse() * (grab - panel.pose.position),
            direction: (inverse * to_grab).normalize_or(Vec3::NEG_Z),
        }
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

    pub fn place(&self, head: &Head, panel: &mut Panel) {
        let rotation = head.rotation();
        let grab = head.position + rotation * self.direction * self.distance;
        panel.pose.rotation = rotation * self.rotation;
        panel.pose.position = grab - panel.pose.rotation * self.grab;
    }
}
