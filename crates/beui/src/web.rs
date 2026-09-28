use std::error::Error;

use beui_adapter_web::{RunOptions, WebRenderer};
use beui_core::app::{App, Setup};
use beui_core::color::Color32;
use beui_core::context::FrameOutput;
use beui_core::geometry::Vec2;
use beui_renderer_wgpu::canvas::CanvasSurface;

struct Canvas(CanvasSurface);

impl WebRenderer for Canvas {
    fn provide(&self, setup: &mut Setup) {
        setup.provide(self.0.gpu());
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.0.resize(width, height);
    }

    fn draw(
        &mut self,
        output: &FrameOutput,
        physical: Vec2,
        scale: f32,
        background: Color32,
    ) -> bool {
        self.0.draw(output, physical, scale, background)
    }
}

pub async fn run_web(
    canvas_id: &str,
    options: RunOptions,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    beui_adapter_web::run_web(
        canvas_id,
        options,
        crate::context(),
        app,
        async |canvas, context| Ok(Canvas(CanvasSurface::new(canvas, context).await?)),
    )
    .await
}

pub fn accessibility_tree() -> Option<String> {
    beui_adapter_web::accessibility_tree()
}
