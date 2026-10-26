use std::time::Instant;

use glam::{Quat, Vec3};

#[cfg(any(windows, target_os = "linux"))]
mod controllers;
#[cfg(any(windows, target_os = "linux"))]
mod gpu;
#[cfg(any(windows, target_os = "linux"))]
mod headset;
mod host;
#[cfg(any(windows, target_os = "linux"))]
mod runtime;
#[cfg(not(any(windows, target_os = "linux")))]
mod unsupported;

#[cfg(any(windows, target_os = "linux"))]
pub use headset::{Headset, RETRY, Xr};
pub use host::{IDLE_POLL, XrSpace};
#[cfg(not(any(windows, target_os = "linux")))]
pub use unsupported::{Headset, RETRY, Xr};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Fov {
    pub left: f32,
    pub right: f32,
    pub up: f32,
    pub down: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct View {
    pub position: Vec3,
    pub orientation: Quat,
    pub fov: Fov,
    pub size: (u32, u32),
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub display: Instant,
    pub views: Vec<View>,
    pub format: wgpu::TextureFormat,
    pub controllers: Vec<beui_core::runner::Controller>,
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

pub struct Target<'a> {
    pub view: usize,
    pub texture: &'a wgpu::Texture,
}
