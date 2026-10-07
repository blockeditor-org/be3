use std::error::Error;

use beui_core::app::Setup;
use beui_core::color::Color32;
use beui_core::context::{FrameOutput, RendererInfo};
use beui_core::geometry::{Vec2, vec2};
use beui_core::renderer::Renderer;

use crate::present::{Gpu, OpenDevice, open_gpu};
use crate::{Repaint, clear_color_in};

const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct OffscreenSurface {
    gpu: Gpu,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    prepared: bool,
}

impl OffscreenSurface {
    pub async fn new(
        width: u32,
        height: u32,
        open_device: Option<OpenDevice>,
    ) -> Result<Self, Box<dyn Error>> {
        let instance = wgpu::Instance::default();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
            })
            .await?;
        let gpu = open_gpu(instance, adapter, FORMAT, open_device).await?;
        let (texture, view) = target(&gpu.device, width, height);
        Ok(Self {
            gpu,
            texture,
            view,
            prepared: false,
        })
    }
}

fn target(device: &wgpu::Device, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("beui offscreen frame"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
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

    fn resize(&mut self, width: u32, height: u32) {
        (self.texture, self.view) = target(&self.gpu.device, width, height);
        self.prepared = false;
    }

    fn physical(&self) -> Option<Vec2> {
        Some(vec2(
            self.texture.width() as f32,
            self.texture.height() as f32,
        ))
    }

    fn prepare(&mut self, output: &FrameOutput, scale: f32, _background: Color32) -> bool {
        if output.changed || !self.prepared {
            let physical = self.physical().unwrap_or(Vec2::ZERO);
            self.gpu.renderer.prepare(
                &self.gpu.device,
                &self.gpu.queue,
                output,
                physical,
                scale,
                Repaint::Everything,
            );
            self.prepared = true;
            return true;
        }
        false
    }

    fn present(&mut self, background: Color32) -> bool {
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("beui offscreen encoder"),
            });
        let size = (self.texture.width(), self.texture.height());
        self.gpu.renderer.render(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &self.view,
            size,
            wgpu::LoadOp::Clear(clear_color_in(FORMAT, background)),
        );
        self.gpu.queue.submit(Some(encoder.finish()));
        false
    }
}
