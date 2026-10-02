use std::error::Error;

use beui_adapter_web::{Loaded, RunOptions};
use beui_core::app::App;

pub enum WebRenderer {
    #[cfg(feature = "web")]
    Wgpu,
    #[cfg(feature = "dom")]
    Dom { icons_font: Option<String> },
}

pub async fn run_web(
    element_id: &str,
    renderers: Vec<WebRenderer>,
    options: RunOptions,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    beui_adapter_web::run_web(element_id, options, app, async |element| {
        let mut loaded = Vec::new();
        let mut failure = None;
        for renderer in renderers {
            match load(renderer, &element).await {
                Ok(renderer) => loaded.push(renderer),
                Err(error) => {
                    web_sys::console::warn_1(&error.to_string().into());
                    failure.get_or_insert(error);
                }
            }
        }
        match (loaded.is_empty(), failure) {
            (true, Some(error)) => Err(error),
            _ => Ok(loaded),
        }
    })
    .await
}

async fn load(
    renderer: WebRenderer,
    element: &web_sys::HtmlElement,
) -> Result<Loaded, Box<dyn Error>> {
    match renderer {
        #[cfg(feature = "web")]
        WebRenderer::Wgpu => load_wgpu(element).await,
        #[cfg(feature = "dom")]
        WebRenderer::Dom { icons_font } => {
            beui_font_browser::watch(icons_font.as_deref(), beui_adapter_web::request_frame)?;
            Ok(Loaded {
                renderer: Box::new(beui_renderer_dom::DomRenderer::new(element.clone())?),
                fonts: Box::new(beui_font_browser::BrowserFonts::default()),
            })
        }
    }
}

#[cfg(feature = "web")]
async fn load_wgpu(element: &web_sys::HtmlElement) -> Result<Loaded, Box<dyn Error>> {
    use wasm_bindgen::JsCast;

    let (canvas, created) = match element.clone().dyn_into::<web_sys::HtmlCanvasElement>() {
        Ok(canvas) => (canvas, false),
        Err(_) => {
            let canvas = element
                .owner_document()
                .ok_or("the element is not in a document")?
                .create_element("canvas")
                .map_err(|_| "could not create a canvas")?
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .map_err(|_| "could not create a canvas")?;
            canvas
                .style()
                .set_css_text("display:block;width:100%;height:100%");
            element
                .append_child(&canvas)
                .map_err(|_| "could not add a canvas to the element")?;
            (canvas, true)
        }
    };
    match beui_renderer_wgpu::canvas::CanvasSurface::new(canvas.clone()).await {
        Ok(surface) => Ok(Loaded {
            renderer: Box::new(surface),
            fonts: Box::new(crate::FreetypeFonts::default()),
        }),
        Err(error) => {
            if created {
                canvas.remove();
            }
            Err(error)
        }
    }
}
