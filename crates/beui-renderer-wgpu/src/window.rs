use std::error::Error;
use std::sync::Arc;

use beui_core::app::Setup;
use beui_core::color::Color32;
use beui_core::context::{FrameOutput, RendererInfo};
use beui_core::geometry::Vec2;
use beui_core::renderer::{Renderer, WindowHandle};

use crate::present::{Gpu, OpenDevice, Presented, Target, create_gpu};

pub struct WindowSurface {
    gpu: Gpu,
    target: Target,
}

impl WindowSurface {
    pub async fn new(
        window: Arc<dyn WindowHandle>,
        open_device: Option<OpenDevice>,
    ) -> Result<Self, Box<dyn Error>> {
        let instance = wgpu::Instance::default();
        let probe = instance.create_surface(window)?;
        let gpu = create_gpu(instance, &probe, open_device).await?;
        drop(probe);
        Ok(Self {
            target: Target::new(gpu.format),
            gpu,
        })
    }
}

impl Renderer for WindowSurface {
    fn name(&self) -> &'static str {
        "wgpu"
    }

    fn info(&self) -> RendererInfo {
        self.gpu.info()
    }

    fn provide(&self, setup: &mut Setup) {
        setup.provide(self.gpu.setup());
    }

    fn attach(
        &mut self,
        window: Arc<dyn WindowHandle>,
        width: u32,
        height: u32,
    ) -> Result<(), Box<dyn Error>> {
        self.target.detach();
        let surface = self.gpu.instance.create_surface(window)?;
        self.target.attach(&self.gpu, surface, width, height)
    }

    fn detach(&mut self) {
        self.target.detach();
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.target.resize(&self.gpu, width, height);
    }

    fn physical(&self) -> Option<Vec2> {
        self.target.physical()
    }

    fn prepare(&mut self, output: &FrameOutput, scale: f32, background: Color32) -> bool {
        self.target
            .prepare(&mut self.gpu, output, scale, background)
    }

    fn present(&mut self, background: Color32) -> bool {
        matches!(
            self.target.present(&mut self.gpu, background),
            Presented::Again
        )
    }
}
