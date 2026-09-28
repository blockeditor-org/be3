use std::error::Error;

use beui_adapter_web::{RunOptions, WebRenderer};
use beui_core::app::{App, Setup};
use beui_core::color::Color32;
use beui_core::context::FrameOutput;
use beui_core::geometry::Vec2;
use beui_renderer_wgpu::canvas::CanvasSurface;
use wasm_bindgen::JsCast;

struct Canvas {
    element: web_sys::HtmlCanvasElement,
    surface: CanvasSurface,
}

impl WebRenderer for Canvas {
    fn provide(&self, setup: &mut Setup) {
        setup.provide(self.surface.gpu());
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.element.set_width(width);
        self.element.set_height(height);
        self.surface.resize(width, height);
    }

    fn draw(
        &mut self,
        output: &FrameOutput,
        physical: Vec2,
        scale: f32,
        background: Color32,
    ) -> bool {
        self.surface.draw(output, physical, scale, background)
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
        async |element, context| {
            let element = element
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(|_| format!("the element {canvas_id} is not a canvas"))?;
            Ok(Canvas {
                surface: CanvasSurface::new(element.clone(), context).await?,
                element,
            })
        },
    )
    .await
}
