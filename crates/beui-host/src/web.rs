use std::error::Error;

use beui_core::app::App;
use beui_platform_web::Web;
use beui_runner::renderer::Loaded;
use beui_runner::{Launch, Layer, Platform, RunOptions};

pub async fn run_web(
    element_id: &str,
    options: RunOptions,
    layers: Vec<Box<dyn Layer>>,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let launch = Launch {
        options,
        context: crate::native::context(),
        app: Box::new(app),
        layers,
    };
    web_platform(element_id).run(launch).await
}

pub fn web_platform(element_id: &str) -> Box<dyn Platform> {
    Box::new(Web::new(element_id, async move |element| {
        load_wgpu(&element).await
    }))
}

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
