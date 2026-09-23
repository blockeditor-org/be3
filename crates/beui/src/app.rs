use std::sync::Arc;

use crate::color::Color32;
use crate::context::Context;
use crate::geometry::{Rect, Vec2};

#[cfg(feature = "window")]
mod clipboard;
#[cfg(feature = "window")]
mod native;
#[cfg(feature = "web")]
mod web;

#[cfg(feature = "window")]
pub use native::{run, run_with};
#[cfg(feature = "web")]
pub use web::run_web;

pub trait App {
    fn update(&mut self, context: &Context, rect: Rect);

    fn clear_color(&self) -> Color32 {
        Color32::BLACK
    }

    fn setup(&mut self, _setup: &Setup) {}

    fn close_requested(&mut self) -> bool {
        true
    }

    fn exiting(&mut self) {}
}

pub struct Setup {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
    pub waker: Waker,
    #[cfg(feature = "window")]
    pub window: Arc<winit::window::Window>,
}

#[derive(Clone)]
pub struct Waker(Arc<dyn Fn() + Send + Sync>);

impl Waker {
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self(Arc::new(wake))
    }

    pub fn wake(&self) {
        (self.0)();
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Waker")
    }
}

pub struct RunOptions {
    pub title: String,
    pub app_id: Option<String>,
    pub size: Vec2,
    #[cfg(all(feature = "window", target_os = "android"))]
    pub android_app: Option<winit::platform::android::activity::AndroidApp>,
}

impl RunOptions {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            app_id: None,
            size: Vec2::new(1280.0, 800.0),
            #[cfg(all(feature = "window", target_os = "android"))]
            android_app: None,
        }
    }
}
