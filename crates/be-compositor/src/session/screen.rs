use std::rc::Rc;

use beui::{FrameOutput, Rect, Renderer, Repaint, Vec2, vec2};

use crate::app::Sprite;
use crate::render::{Gpu, SurfaceTexture};

pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

pub struct Frame<'a> {
    pub output: &'a FrameOutput,
    pub scale: f32,
    pub clear: beui::Color32,
    pub pointer: beui::Pos2,
    pub sprite: &'a Sprite,
    pub arrow: &'a SurfaceTexture,
}

pub struct Screen {
    size: (u32, u32),
    pub rect: Rect,
    renderer: Renderer,
    retained: wgpu::Texture,
    pending: Option<Repaint>,
    pub cursor: bool,
}

impl Screen {
    pub fn new(gpu: &Gpu, size: (u32, u32)) -> Self {
        Self {
            size,
            rect: Rect::ZERO,
            renderer: Renderer::new(gpu.device(), FORMAT),
            retained: retained(gpu.device(), size),
            pending: Some(Repaint::Everything),
            cursor: false,
        }
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    pub fn invalidate(&mut self) {
        self.pending = Some(Repaint::Everything);
    }

    pub fn damage(&mut self, repaint: Repaint) {
        self.pending = Some(match self.pending.take() {
            Some(pending) => pending.union(repaint),
            None => repaint,
        });
    }

    pub fn dirty(&self) -> bool {
        self.pending.is_some() || self.cursor
    }

    pub fn replace_gpu(&mut self, gpu: &Gpu) {
        self.renderer = Renderer::new(gpu.device(), FORMAT);
        self.retained = retained(gpu.device(), self.size);
        self.invalidate();
    }

    pub fn draw(
        &mut self,
        gpu: &Gpu,
        frame: &Frame<'_>,
        target: &wgpu::Texture,
    ) -> wgpu::SubmissionIndex {
        let device = gpu.device();
        let queue = gpu.queue();
        let scale = frame.scale;
        let physical = vec2(self.size.0 as f32, self.size.1 as f32);
        self.renderer
            .set_origin(vec2(self.rect.min.x * scale, self.rect.min.y * scale));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("output"),
        });
        if let Some(repaint) = self.pending.take() {
            let effective =
                self.renderer
                    .prepare(device, queue, frame.output, physical, scale, repaint);
            let load = match effective {
                Repaint::Region { .. } => wgpu::LoadOp::Load,
                Repaint::Everything => wgpu::LoadOp::Clear(beui::clear_color(frame.clear)),
            };
            let view = self
                .retained
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.renderer
                .render(device, queue, &mut encoder, &view, self.size, load);
        }
        encoder.copy_texture_to_texture(
            self.retained.as_image_copy(),
            target.as_image_copy(),
            wgpu::Extent3d {
                width: self.size.0,
                height: self.size.1,
                depth_or_array_layers: 1,
            },
        );
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        self.paint_cursor(gpu, &mut encoder, &view, frame);
        self.cursor = false;
        queue.submit([encoder.finish()])
    }

    fn paint_cursor(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        frame: &Frame<'_>,
    ) {
        let (texture, size, hotspot): (&SurfaceTexture, Vec2, Vec2) = match frame.sprite {
            Sprite::Hidden => return,
            Sprite::Arrow => (
                frame.arrow,
                vec2(super::arrow::WIDTH as f32, super::arrow::HEIGHT as f32),
                Vec2::ZERO,
            ),
            Sprite::Client {
                texture,
                size,
                hotspot,
            } => (Rc::as_ref(texture), *size, *hotspot),
        };
        let scale = frame.scale;
        let left = (frame.pointer.x - hotspot.x - self.rect.min.x) * scale;
        let top = (frame.pointer.y - hotspot.y - self.rect.min.y) * scale;
        let rect = [left, top, left + size.x * scale, top + size.y * scale];
        let (width, height) = (self.size.0 as f32, self.size.1 as f32);
        if rect[2] <= 0.0 || rect[3] <= 0.0 || rect[0] >= width || rect[1] >= height {
            return;
        }
        gpu.paint_sprite(encoder, view, self.size, rect, texture);
    }
}

fn retained(device: &wgpu::Device, size: (u32, u32)) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("output frame"),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

#[cfg(test)]
mod tests;
