use crate::app::Setup;
use crate::color::Color32;
use crate::context::{FrameOutput, RendererInfo};
use crate::geometry::Vec2;

pub trait Renderer {
    fn name(&self) -> &'static str;

    fn info(&self) -> RendererInfo;

    fn provide(&self, _setup: &mut Setup) {}

    fn set_active(&mut self, active: bool);

    fn resize(&mut self, width: u32, height: u32);

    fn draw(
        &mut self,
        output: &FrameOutput,
        physical: Vec2,
        scale: f32,
        background: Color32,
    ) -> bool;
}
