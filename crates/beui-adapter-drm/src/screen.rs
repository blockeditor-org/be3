use beui::{FrameOutput, Rect, Renderer, Repaint, Vec2, vec2};

use crate::gpu::{Gpu, Sprite};

pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

pub enum Pointer<'a> {
    Hidden,
    Sprite {
        sprite: &'a Sprite,
        size: Vec2,
        hotspot: Vec2,
    },
}

pub struct Frame<'a> {
    pub scale: f32,
    pub clear: beui::Color32,
    pub pointer: beui::Pos2,
    pub sprite: Pointer<'a>,
}

pub struct Screen {
    size: (u32, u32),
    pub rect: Rect,
    renderer: Renderer,
    retained: wgpu::Texture,
    pending: Option<Repaint>,
    prepared: Option<Repaint>,
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
            prepared: None,
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
        self.prepared.is_some() || self.cursor
    }

    pub fn prepare(&mut self, gpu: &Gpu, output: &FrameOutput, scale: f32) {
        let Some(repaint) = self.pending else {
            return;
        };
        let physical = vec2(self.size.0 as f32, self.size.1 as f32);
        self.renderer
            .set_origin(vec2(self.rect.min.x * scale, self.rect.min.y * scale));
        self.prepared = Some(self.renderer.prepare(
            gpu.device(),
            gpu.queue(),
            output,
            physical,
            scale,
            repaint,
        ));
    }

    pub fn draw(
        &mut self,
        gpu: &Gpu,
        frame: &Frame<'_>,
        target: &wgpu::Texture,
    ) -> wgpu::CommandBuffer {
        let device = gpu.device();
        let queue = gpu.queue();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("output"),
        });
        if let Some(effective) = self.prepared.take() {
            self.pending = None;
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
        encoder.finish()
    }

    fn paint_cursor(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        frame: &Frame<'_>,
    ) {
        let (sprite, size, hotspot) = match &frame.sprite {
            Pointer::Hidden => return,
            Pointer::Sprite {
                sprite,
                size,
                hotspot,
            } => (*sprite, *size, *hotspot),
        };
        let scale = frame.scale;
        let left = (frame.pointer.x - hotspot.x - self.rect.min.x) * scale;
        let top = (frame.pointer.y - hotspot.y - self.rect.min.y) * scale;
        let rect = [left, top, left + size.x * scale, top + size.y * scale];
        let (width, height) = (self.size.0 as f32, self.size.1 as f32);
        if rect[2] <= 0.0 || rect[3] <= 0.0 || rect[0] >= width || rect[1] >= height {
            return;
        }
        gpu.paint_sprite(encoder, view, self.size, rect, sprite);
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
