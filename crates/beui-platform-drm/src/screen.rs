use beui::{Rect, Vec2, native::WindowOutput, vec2};
use beui_host::{native::Renderer, native::Repaint};
use smithay::utils::{Physical, Rectangle};

use crate::gpu::{Gpu, Sprite};
use crate::rotation::{Rotation, Turn};

pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8Unorm;

pub enum Pointer<'a> {
    Hidden,
    Sprite {
        sprite: &'a Sprite,
        size: Vec2,
        hotspot: Vec2,
    },
}

pub type Damage = Option<Vec<Rectangle<i32, Physical>>>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Drawn {
    #[default]
    Nothing,
    Area(Rectangle<i32, Physical>),
    Everything,
}

impl Drawn {
    fn of(damage: &Damage) -> Self {
        match damage {
            None => Self::Everything,
            Some(rects) => rects
                .iter()
                .fold(Self::Nothing, |drawn, rect| drawn.union(Self::Area(*rect))),
        }
    }

    fn union(self, other: Self) -> Self {
        match (self, other) {
            (Self::Nothing, drawn) | (drawn, Self::Nothing) => drawn,
            (Self::Area(area), Self::Area(other)) => Self::Area(area.merge(other)),
            _ => Self::Everything,
        }
    }

    fn add_to(self, damage: Damage) -> Damage {
        match self {
            Self::Nothing => damage,
            Self::Everything => None,
            Self::Area(area) => damage.map(|mut rects| {
                rects.push(area);
                rects
            }),
        }
    }
}

pub struct Frame<'a> {
    pub clear: beui::Color32,
    pub pointer: beui::Pos2,
    pub sprite: Pointer<'a>,
}

pub struct Screen {
    size: (u32, u32),
    rotation: Rotation,
    picture: (u32, u32),
    pub rect: Rect,
    pub scale: f32,
    pub zoom: f32,
    renderer: Renderer,
    retained: wgpu::Texture,
    turned: Option<Sprite>,
    pending: Option<Repaint>,
    prepared: Option<Repaint>,
    pub cursor: bool,
    drawn_cursor: Option<Rectangle<i32, Physical>>,
    last: Drawn,
    hurried: Option<Drawn>,
    settling: Drawn,
}

impl Screen {
    pub fn new(gpu: &Gpu, size: (u32, u32), rotation: Rotation) -> Self {
        let picture = rotation.turned(size);
        let retained = retained(gpu.device(), picture);
        let turned = (rotation != Rotation::Normal).then(|| gpu.sprite(&retained, true));
        Self {
            size,
            rotation,
            picture,
            rect: Rect::ZERO,
            scale: 1.0,
            zoom: 1.0,
            renderer: Renderer::new(gpu.device(), FORMAT),
            retained,
            turned,
            pending: Some(Repaint::Everything),
            prepared: None,
            cursor: false,
            drawn_cursor: None,
            last: Drawn::Nothing,
            hurried: None,
            settling: Drawn::Nothing,
        }
    }

    pub fn picture(&self) -> (u32, u32) {
        self.picture
    }

    pub fn rotation(&self) -> Rotation {
        self.rotation
    }

    fn picture_size(&self) -> Vec2 {
        vec2(self.picture.0 as f32, self.picture.1 as f32)
    }

    fn on_panel(&self, rect: [f32; 4]) -> [f32; 4] {
        self.rotation.rect_to_panel(self.picture_size(), rect)
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

    pub fn hurry(&mut self) {
        self.hurried = Some(self.last);
    }

    pub fn settle(&mut self) {
        if let Some(hurried) = self.hurried.take() {
            self.settling = self.settling.union(hurried);
        }
    }

    pub fn dirty(&self) -> bool {
        self.prepared.is_some() || self.cursor || self.settling != Drawn::Nothing
    }

    pub fn prepare(&mut self, gpu: &Gpu, output: &WindowOutput) {
        let Some(repaint) = self.pending else {
            return;
        };
        let physical = self.picture_size();
        self.prepared = Some(self.renderer.prepare(
            gpu.device(),
            gpu.queue(),
            output,
            physical,
            self.scale * self.zoom,
            repaint,
        ));
    }

    pub fn size(&self) -> Vec2 {
        self.picture_size() / self.scale
    }

    pub fn draw(
        &mut self,
        gpu: &Gpu,
        frame: &Frame<'_>,
        target: &wgpu::Texture,
    ) -> (wgpu::CommandBuffer, Damage) {
        let device = gpu.device();
        let queue = gpu.queue();
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("output"),
        });
        let mut damage = Some(Vec::new());
        if let Some(effective) = self.prepared.take() {
            self.pending = None;
            damage = match (effective, self.renderer.scissors()) {
                (Repaint::Region { .. }, Some(scissors)) => Some(
                    scissors
                        .iter()
                        .map(|[x, y, width, height]| {
                            let [left, top, right, bottom] = self.on_panel([
                                *x as f32,
                                *y as f32,
                                (*x + *width) as f32,
                                (*y + *height) as f32,
                            ]);
                            Rectangle::new(
                                (left as i32, top as i32).into(),
                                ((right - left) as i32, (bottom - top) as i32).into(),
                            )
                        })
                        .collect(),
                ),
                _ => None,
            };
            let load = match effective {
                Repaint::Region { .. } => wgpu::LoadOp::Load,
                Repaint::Everything => wgpu::LoadOp::Clear(beui_host::native::wgpu_color(frame.clear)),
            };
            let view = self
                .retained
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.renderer
                .render(device, queue, &mut encoder, &view, self.picture, load);
        }
        let view = target.create_view(&wgpu::TextureViewDescriptor::default());
        match &self.turned {
            None => encoder.copy_texture_to_texture(
                self.retained.as_image_copy(),
                target.as_image_copy(),
                wgpu::Extent3d {
                    width: self.size.0,
                    height: self.size.1,
                    depth_or_array_layers: 1,
                },
            ),
            Some(turned) => {
                let (width, height) = (self.size.0 as f32, self.size.1 as f32);
                gpu.paint_sprite(
                    &mut encoder,
                    &view,
                    self.size,
                    [0.0, 0.0, width, height],
                    turned,
                    self.rotation,
                );
            }
        }
        let cursor = self.paint_cursor(gpu, &mut encoder, &view, frame);
        if let Some(damage) = &mut damage {
            damage.extend(self.drawn_cursor.into_iter().chain(cursor));
        }
        self.drawn_cursor = cursor;
        self.cursor = false;
        self.last = Drawn::of(&damage);
        if let Some(hurried) = &mut self.hurried {
            *hurried = hurried.union(self.last);
        }
        let damage = std::mem::take(&mut self.settling).add_to(damage);
        (encoder.finish(), damage)
    }

    fn paint_cursor(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        frame: &Frame<'_>,
    ) -> Option<Rectangle<i32, Physical>> {
        let (sprite, size, hotspot) = match &frame.sprite {
            Pointer::Hidden => return None,
            Pointer::Sprite {
                sprite,
                size,
                hotspot,
            } => (*sprite, *size, *hotspot),
        };
        let scale = self.scale;
        let left = (frame.pointer.x - hotspot.x - self.rect.min.x) * scale;
        let top = (frame.pointer.y - hotspot.y - self.rect.min.y) * scale;
        let rect = [left, top, left + size.x * scale, top + size.y * scale];
        let picture = self.picture_size();
        if rect[2] <= 0.0 || rect[3] <= 0.0 || rect[0] >= picture.x || rect[1] >= picture.y {
            return None;
        }
        let rect = self.on_panel(rect);
        let (width, height) = (self.size.0 as f32, self.size.1 as f32);
        gpu.paint_sprite(encoder, view, self.size, rect, sprite, self.rotation);
        let left = rect[0].floor().max(0.0) as i32;
        let top = rect[1].floor().max(0.0) as i32;
        let right = rect[2].ceil().min(width) as i32;
        let bottom = rect[3].ceil().min(height) as i32;
        Some(Rectangle::new(
            (left, top).into(),
            (right - left, bottom - top).into(),
        ))
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
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

#[cfg(test)]
mod tests;
