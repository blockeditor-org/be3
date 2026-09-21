use std::any::Any;
use std::rc::Rc;

#[cfg(feature = "render")]
use crate::geometry::Vec2;

#[cfg(feature = "render")]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawAt {
    pub rect: [f32; 4],
    pub clip: [f32; 4],
    pub screen: Vec2,
    pub pixels_per_point: f32,
    pub format: wgpu::TextureFormat,
}

#[cfg(feature = "render")]
impl DrawAt {
    pub fn width(&self) -> u32 {
        (self.rect[2] - self.rect[0]).max(0.0) as u32
    }

    pub fn height(&self) -> u32 {
        (self.rect[3] - self.rect[1]).max(0.0) as u32
    }
}

#[cfg(feature = "render")]
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

#[derive(Clone)]
pub struct Drawing(Rc<dyn Any>);

impl Drawing {
    #[cfg(feature = "render")]
    pub fn new(draw: impl Draw) -> Self {
        let draw: Rc<dyn Draw> = Rc::new(draw);
        Self(Rc::new(draw))
    }

    #[cfg(feature = "render")]
    pub(crate) fn draw(&self) -> Option<&Rc<dyn Draw>> {
        self.0.downcast_ref::<Rc<dyn Draw>>()
    }
}

impl PartialEq for Drawing {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for Drawing {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Drawing")
            .field("at", &Rc::as_ptr(&self.0).cast::<()>())
            .finish()
    }
}
