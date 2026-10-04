use std::error::Error;

use beui_adapter_web::Web;
use beui_core::app::App;
use beui_core::renderer::{Loaded, any_loaded};
use beui_core::runner::{Adapter, Launch, RunOptions};

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
    let launch = Launch {
        options,
        context: crate::context(),
        app: Box::new(app),
    };
    web_adapter(element_id, renderers).run(launch).await
}

pub fn web_adapter(element_id: &str, renderers: Vec<WebRenderer>) -> Box<dyn Adapter> {
    Box::new(Web::new(element_id, async move |element| {
        let mut results = Vec::new();
        for renderer in renderers {
            results.push(load(renderer, &element).await);
        }
        any_loaded(results, |error| {
            web_sys::console::warn_1(&format!("beui: a renderer did not load: {error}").into());
        })
    }))
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
            if element.tag_name().eq_ignore_ascii_case("canvas") {
                return Err("the DOM renderer cannot draw inside a canvas".into());
            }
            beui_font_browser::watch(icons_font.as_deref(), beui_adapter_web::request_frame)?;
            Ok(Loaded {
                renderer: Box::new(beui_renderer_dom::DomRenderer::new(element.clone())?),
                fonts: Some(Box::new(beui_font_browser::BrowserFonts::default())),
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
            fonts: None,
        }),
        Err(error) => {
            if created {
                canvas.remove();
            }
            Err(error)
        }
    }
}
