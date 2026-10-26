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
    pub layers: Layers,
}

pub const MOST_LAYERS: usize = 8;

#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Layers {
    pub most: usize,
    pub cylinders: bool,
    pub largest: (u32, u32),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Quad {
    pub window: beui_core::window::WindowId,
    pub position: Vec3,
    pub orientation: Quat,
    pub metres: (f32, f32),
    pub bend: f32,
    pub pixels: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub version: u64,
}

pub struct Fill<'a> {
    pub quad: usize,
    pub texture: Option<&'a wgpu::Texture>,
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

pub struct Target<'a> {
    pub view: usize,
    pub texture: &'a wgpu::Texture,
}
