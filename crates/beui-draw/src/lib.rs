use std::rc::Rc;

use beui_core::drawing::Drawing;
use beui_core::painting::Paints;
use beui_types::Vec2;

pub mod compose;
pub mod texture_quad;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawAt {
    pub rect: [f32; 4],
    pub clip: [f32; 4],
    pub screen: Vec2,
    pub pixels_per_point: f32,
    pub format: wgpu::TextureFormat,
    pub renderer: u64,
    /// Whether the renderer composites this frame (`compose`): a drawing that shows a surface
    /// then lists it as a layer (`Draw::layers`), and leaves one that lies over the UI (an
    /// anchored view) for the renderer to composite rather than painting it.
    pub composited: bool,
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

    /// The surfaces this drawing shows as layers of its window when its renderer composites
    /// (`DrawAt::composited`), bottom first, as of its last `prepare`. A composited renderer
    /// asks after each `prepare`, and keeps the answer while the drawing stays where it was.
    fn layers(&self) -> Vec<compose::Placed> {
        Vec::new()
    }
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
