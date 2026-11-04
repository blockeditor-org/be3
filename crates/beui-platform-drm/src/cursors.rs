//! The pointer's shapes: `beui_core::drawn_cursor`'s icons, rendered once per icon at the
//! displays' scale. A login session has no system cursor to show, so the session draws the one
//! space and mouse simulation draw (decisions/our_own_cursors_where_we_draw_them.md).

use be_lib_stable_hash::HashMap;
use std::rc::Rc;

use beui::native::WindowOutput;
use beui::{Color32, CursorIcon, Pos2, Vec2};
use beui_core::context::Context;
use beui_core::drawn_cursor;
use beui_host::native::{Renderer, Repaint};

use crate::gpu::{Gpu, Sprite};
use crate::screen::FORMAT;

pub const COLOR: Color32 = Color32::WHITE;

pub struct Shape {
    pub sprite: Sprite,
    pub size: Vec2,
    pub hotspot: Vec2,
}

pub struct Cursors {
    context: Context,
    renderer: Option<Renderer>,
    scale: f32,
    loaded: HashMap<CursorIcon, Option<Rc<Shape>>>,
}

impl Cursors {
    pub fn new(scale: f32) -> Self {
        let context = beui_host::native::context();
        context.set_pixels_per_point(scale);
        Self {
            context,
            renderer: None,
            scale,
            loaded: HashMap::default(),
        }
    }

    pub fn get(&mut self, gpu: &Gpu, icon: CursorIcon) -> Option<Rc<Shape>> {
        if let Some(shape) = self.loaded.get(&icon) {
            return shape.clone();
        }
        let shape = (icon != CursorIcon::None).then(|| Rc::new(self.draw(gpu, icon)));
        self.loaded.insert(icon, shape.clone());
        shape
    }

    fn draw(&mut self, gpu: &Gpu, icon: CursorIcon) -> Shape {
        let output = picture(&self.context, icon);
        let side = (drawn_cursor::EXTENT * self.scale).ceil().max(1.0) as u32;
        let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("cursor"),
            size: wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let renderer = self
            .renderer
            .get_or_insert_with(|| Renderer::new(gpu.resources()));
        renderer.prepare(
            gpu.device(),
            gpu.queue(),
            &output,
            Vec2::splat(side as f32),
            self.scale,
            Repaint::Everything,
        );
        let mut encoder = gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cursor"),
            });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        renderer.render(
            gpu.device(),
            gpu.queue(),
            &mut encoder,
            &view,
            (side, side),
            wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
        );
        gpu.queue().submit([encoder.finish()]);
        Shape {
            sprite: gpu.sprite(&texture, false),
            size: Vec2::splat(drawn_cursor::EXTENT),
            hotspot: hotspot().to_vec2(),
        }
    }
}

fn hotspot() -> Pos2 {
    Pos2::new(drawn_cursor::EXTENT / 2.0, drawn_cursor::EXTENT / 2.0)
}

/// `icon` as `drawn_cursor` paints it, with its hotspot in the middle of the picture.
pub fn picture(context: &Context, icon: CursorIcon) -> WindowOutput {
    let mut output = context.run(|context| {
        drawn_cursor::paint(&context.painter(), hotspot(), icon, COLOR);
    });
    let mut window = output.windows.swap_remove(0);
    window.size = Vec2::splat(drawn_cursor::EXTENT);
    window.window.appearance.clear_color = Color32::TRANSPARENT;
    window
}

#[cfg(test)]
mod tests;
