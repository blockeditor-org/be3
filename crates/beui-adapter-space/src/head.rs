use beui_core::geometry::Vec2;
use glam::{Mat4, Quat, Vec3};

pub const EYE_HEIGHT: f32 = 1.6;
pub const FOV_Y: f32 = std::f32::consts::FRAC_PI_3;
pub const NEAR: f32 = 0.05;
pub const FAR: f32 = 200.0;
pub const TURN_PER_PIXEL: f32 = 0.0025;
pub const MAX_PITCH: f32 = 1.5;
pub const WALK_SPEED: f32 = 1.5;
pub const ROLL_PER_SECOND: f32 = 1.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn at(&self, distance: f32) -> Vec3 {
        self.origin + self.direction * distance
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Head {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
}

impl Default for Head {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, EYE_HEIGHT, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        }
    }
}

impl Head {
    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(-self.yaw)
            * Quat::from_rotation_x(self.pitch)
            * Quat::from_rotation_z(self.roll)
    }

    pub fn facing(&self) -> Quat {
        Quat::from_rotation_y(-self.yaw)
    }

    pub fn look(&mut self, motion: Vec2) {
        let motion = glam::Vec2::from_angle(-self.roll).rotate(glam::Vec2::new(motion.x, motion.y));
        self.yaw += motion.x * TURN_PER_PIXEL;
        self.pitch = (self.pitch - motion.y * TURN_PER_PIXEL).clamp(-MAX_PITCH, MAX_PITCH);
    }

    pub fn tilt(&mut self, roll: f32, seconds: f32) {
        self.roll = (self.roll + roll * ROLL_PER_SECOND * seconds + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
    }

    pub fn walk(&mut self, strafe: f32, forward: f32, seconds: f32) {
        let step = self.facing() * Vec3::new(strafe, 0.0, -forward);
        self.position += step.normalize_or_zero() * WALK_SPEED * seconds;
    }

    pub fn view(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation(), self.position).inverse()
    }

    pub fn projection(screen: Vec2) -> Mat4 {
        Mat4::perspective_rh(FOV_Y, aspect(screen), NEAR, FAR)
    }

    pub fn view_projection(&self, screen: Vec2) -> Mat4 {
        Self::projection(screen) * self.view()
    }

    pub fn direction(screen: Vec2, at: Vec2) -> Vec3 {
        let half = (FOV_Y / 2.0).tan();
        let x = (at.x / screen.x.max(1.0)) * 2.0 - 1.0;
        let y = 1.0 - (at.y / screen.y.max(1.0)) * 2.0;
        Vec3::new(x * half * aspect(screen), y * half, -1.0).normalize()
    }

    pub fn across(&self, screen: Vec2, point: Vec3) -> glam::Vec2 {
        let local = self.rotation().inverse() * (point - self.position);
        let half = (FOV_Y / 2.0).tan();
        let across = glam::Vec2::new(local.x / (half * aspect(screen)), local.y / half);
        let depth = -local.z;
        match depth > 1e-4 {
            true => across / depth,
            false => across.normalize_or(glam::Vec2::NEG_Y) * 1e4,
        }
    }

    pub fn on_screen(screen: Vec2, across: glam::Vec2) -> Vec2 {
        Vec2::new(
            (across.x + 1.0) / 2.0 * screen.x,
            (1.0 - across.y) / 2.0 * screen.y,
        )
    }

    pub fn ray(&self, screen: Vec2, at: Vec2) -> Ray {
        Ray {
            origin: self.position,
            direction: self.rotation() * Self::direction(screen, at),
        }
    }
}

fn aspect(screen: Vec2) -> f32 {
    screen.x.max(1.0) / screen.y.max(1.0)
}
