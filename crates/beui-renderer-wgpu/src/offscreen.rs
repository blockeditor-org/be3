use std::error::Error;

use beui_core::app::Setup;
use beui_core::app::automation::Capture;
use beui_core::color::Color32;
use beui_core::context::{FrameOutput, RendererInfo};
use beui_core::geometry::Vec2;
use beui_core::renderer::Renderer;

use crate::present::{Gpu, OpenDevice, Presented, Target, create_offscreen};

pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct OffscreenSurface {
    gpu: Gpu,
    target: Target,
}

impl OffscreenSurface {
    pub async fn new(
        width: u32,
        height: u32,
        open_device: Option<OpenDevice>,
    ) -> Result<Self, Box<dyn Error>> {
        let gpu = create_offscreen(FORMAT, open_device).await?;
        Ok(Self {
            target: Target::offscreen(&gpu, width, height),
            gpu,
        })
    }
}

impl Renderer for OffscreenSurface {
    fn name(&self) -> &'static str {
        "wgpu"
    }

    fn info(&self) -> RendererInfo {
        self.gpu.info()
    }

    fn provide(&self, setup: &mut Setup) {
        setup.provide(self.gpu.setup());
    }

    fn recover(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.gpu.lost() {
            return Ok(false);
        }
        let (width, height) = self.target.size();
        pollster::block_on(self.gpu.reopen(None))?;
        self.target = Target::offscreen(&self.gpu, width, height);
        Ok(true)
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

    fn capture(&mut self) -> Option<Capture> {
        self.target.capture(&self.gpu)
    }
}
