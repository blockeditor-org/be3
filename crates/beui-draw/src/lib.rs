use std::rc::Rc;

use beui_core::drawing::Drawing;
use beui_core::geometry::Vec2;
use beui_core::painting::Paints;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawAt {
    pub rect: [f32; 4],
    pub clip: [f32; 4],
    pub screen: Vec2,
    pub pixels_per_point: f32,
    pub format: wgpu::TextureFormat,
    pub renderer: u64,
}

impl DrawAt {
    pub fn width(&self) -> u32 {
        (self.rect[2] - self.rect[0]).max(0.0) as u32
    }

    pub fn height(&self) -> u32 {
        (self.rect[3] - self.rect[1]).max(0.0) as u32
    }
}

pub trait Draw: 'static {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _encoder: &mut wgpu::CommandEncoder,
        _at: DrawAt,
    ) {
    }

    fn paint(&self, pass: &mut wgpu::RenderPass<'_>, at: DrawAt);
}

pub fn drawing(draw: impl Draw) -> Drawing {
    let draw: Rc<dyn Draw> = Rc::new(draw);
    Drawing::erased(draw)
}

pub fn painted_drawing(draw: impl Draw + Paints) -> Drawing {
    let draw = Rc::new(draw);
    let paints: Rc<dyn Paints> = draw.clone();
    let draw: Rc<dyn Draw> = draw;
    Drawing::painted(draw, paints)
}
