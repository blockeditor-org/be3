use std::error::Error;

use beui_adapter_web::{RunOptions, WebRenderer};
use beui_core::app::App;
use beui_core::color::Color32;
use beui_core::context::{Context, FrameOutput};
use beui_core::geometry::Vec2;
use beui_font_browser::BrowserFonts;
use beui_renderer_dom::DomRenderer;

struct Dom(DomRenderer);

impl WebRenderer for Dom {
    fn resize(&mut self, _width: u32, _height: u32) {}

    fn draw(
        &mut self,
        output: &FrameOutput,
        _physical: Vec2,
        scale: f32,
        background: Color32,
    ) -> bool {
        self.0.draw(output, scale, background);
        false
    }
}

pub async fn run_dom(
    element_id: &str,
    icons_font: Option<&str>,
    options: RunOptions,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    beui_font_browser::watch(icons_font, beui_adapter_web::request_frame)?;
    beui_adapter_web::run_web(
        element_id,
        options,
        Context::new(BrowserFonts::default()),
        app,
        async |element, context| Ok(Dom(DomRenderer::new(element, context)?)),
    )
    .await
}
