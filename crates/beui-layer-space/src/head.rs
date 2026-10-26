use std::time::Instant;

use beui_core::geometry::Vec2;
use glam::{EulerRot, Mat4, Quat, Vec3, Vec4};

pub const EYE_HEIGHT: f32 = 1.6;
pub const FOV_Y: f32 = std::f32::consts::FRAC_PI_3;
pub const NEAR: f32 = 0.05;
pub const FAR: f32 = 200.0;
pub const TURN_PER_PIXEL: f32 = 0.0025;
pub const MAX_PITCH: f32 = 1.5;
pub const WALK_SPEED: f32 = 1.5;
pub const ROLL_PER_SECOND: f32 = 1.0;
pub const MOST_ELEVATION: f32 = 1.48;

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

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Fov {
    left: f32,
    right: f32,
    up: f32,
    down: f32,
    projection: Mat4,
}

impl Fov {
    pub fn symmetric(screen: Vec2) -> Self {
        let up = (FOV_Y / 2.0).tan();
        let right = up * aspect(screen);
        Self {
            left: -right,
            right,
            up,
            down: -up,
            projection: Mat4::perspective_rh(FOV_Y, aspect(screen), NEAR, FAR),
        }
    }

    pub fn tangents(left: f32, right: f32, up: f32, down: f32) -> Self {
        let (width, height) = (right - left, up - down);
        let depth = FAR / (NEAR - FAR);
        let projection = Mat4::from_cols(
            Vec4::new(2.0 / width, 0.0, 0.0, 0.0),
            Vec4::new(0.0, 2.0 / height, 0.0, 0.0),
            Vec4::new((right + left) / width, (up + down) / height, depth, -1.0),
            Vec4::new(0.0, 0.0, depth * NEAR, 0.0),
        );
        Self {
            left,
            right,
            up,
            down,
            projection,
        }
    }

    pub fn from_angles(left: f32, right: f32, up: f32, down: f32) -> Self {
        Self::tangents(left.tan(), right.tan(), up.tan(), down.tan())
    }

    fn width(&self) -> f32 {
        self.right - self.left
    }

    fn height(&self) -> f32 {
        self.up - self.down
    }

    pub fn projection(&self) -> Mat4 {
        self.projection
    }

    pub fn pixels_per_tangent(&self, size: Vec2) -> f32 {
        (size.x / self.width()).max(size.y / self.height())
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct View {
    pub position: Vec3,
    pub rotation: Quat,
    pub fov: Fov,
    pub size: Vec2,
}

impl View {
    pub fn view(&self) -> Mat4 {
        Mat4::from_rotation_translation(self.rotation, self.position).inverse()
    }

    pub fn view_projection(&self) -> Mat4 {
        self.fov.projection() * self.view()
    }
}

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Body {
    pub position: Vec3,
    pub yaw: f32,
}

impl Body {
    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(-self.yaw)
    }

    pub fn place(&self, position: Vec3, rotation: Quat) -> (Vec3, Quat) {
        (
            self.position + self.rotation() * position,
            self.rotation() * rotation,
        )
    }

    pub fn moved(&self, pivot: Vec3, turn: f32, step: Vec3) -> Self {
        Self {
            position: pivot + Quat::from_rotation_y(-turn) * (self.position - pivot) + step,
            yaw: self.yaw + turn,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Views {
    pub display_time: Instant,
    pub views: Vec<View>,
}

impl Views {
    pub fn placed(mut self, body: &Body) -> Self {
        for view in &mut self.views {
            (view.position, view.rotation) = body.place(view.position, view.rotation);
        }
        self
    }

    pub fn centre(&self) -> Option<(Vec3, Quat)> {
        let first = self.views.first()?;
        let mut position = Vec3::ZERO;
        let mut rotation = first.rotation;
        for (index, view) in self.views.iter().enumerate() {
            position += view.position;
            rotation = rotation.slerp(view.rotation, 1.0 / (index + 1) as f32);
        }
        Some((position / self.views.len() as f32, rotation.normalize()))
    }
}

impl Head {
    pub fn posed(position: Vec3, rotation: Quat) -> Self {
        let (yaw, pitch, roll) = rotation.to_euler(EulerRot::YXZ);
        Self {
            position,
            yaw: -yaw,
            pitch,
            roll,
        }
    }

    pub fn view(&self, screen: Vec2) -> View {
        View {
            position: self.position,
            rotation: self.rotation(),
            fov: Fov::symmetric(screen),
            size: screen,
        }
    }

    pub fn rotation(&self) -> Quat {
        Quat::from_rotation_y(-self.yaw)
            * Quat::from_rotation_x(self.pitch)
            * Quat::from_rotation_z(self.roll)
    }

    pub fn up(&self) -> Vec3 {
        self.rotation() * Vec3::Y
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

    pub fn view_projection(&self, screen: Vec2) -> Mat4 {
        self.view(screen).view_projection()
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

    pub fn pixels_per_radian(screen: Vec2) -> f32 {
        screen.y.max(1.0) / 2.0 / (FOV_Y / 2.0).tan()
    }

    pub fn swing(&self, direction: Vec3, angles: glam::Vec2) -> Vec3 {
        let from = direction.y.clamp(-1.0, 1.0).asin();
        let to = (from + angles.y).clamp(-MOST_ELEVATION, MOST_ELEVATION);
        let level =
            Vec3::new(direction.x, 0.0, direction.z).normalize_or(self.facing() * Vec3::NEG_Z);
        let across = angles.x / ((from + to) / 2.0).cos().max(MOST_ELEVATION.cos());
        let level = Quat::from_rotation_y(-across) * level;
        level * to.cos() + Vec3::Y * to.sin()
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
