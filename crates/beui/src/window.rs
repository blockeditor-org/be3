use std::error::Error;
use std::sync::Arc;

use beui_core::app::App;
use beui_core::renderer::{Loaded, WindowHandle, any_loaded};
use beui_renderer_wgpu::present::OpenDevice;
use beui_renderer_wgpu::window::WindowSurface;

use beui_core::runner::{Adapter, Launch, RunOptions};

pub enum WindowRenderer {
    Wgpu { open_device: Option<OpenDevice> },
}

impl WindowRenderer {
    pub fn wgpu() -> Self {
        Self::Wgpu { open_device: None }
    }
}

pub fn run_with(options: RunOptions, app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    run_with_renderers(options, vec![WindowRenderer::wgpu()], app)
}

pub fn run_with_renderers(
    options: RunOptions,
    renderers: Vec<WindowRenderer>,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let launch = Launch {
        options,
        context: crate::system_context(),
        app: Box::new(app),
    };
    pollster::block_on(window_adapter(renderers).run(launch))
}

pub fn window_adapter(renderers: Vec<WindowRenderer>) -> Box<dyn Adapter> {
    let load = move |window: Arc<dyn WindowHandle>| {
        let results = renderers
            .into_iter()
            .map(|renderer| load(renderer, window.clone()));
        any_loaded(results, |error| {
            eprintln!("beui: a renderer did not load: {error}");
        })
    };
    #[cfg(target_os = "android")]
    return Box::new(beui_adapter_android::Android::new(load));
    #[cfg(not(target_os = "android"))]
    return Box::new(beui_adapter_winit::Winit::new(load));
}

fn load(renderer: WindowRenderer, window: Arc<dyn WindowHandle>) -> Result<Loaded, Box<dyn Error>> {
    match renderer {
        WindowRenderer::Wgpu { open_device } => Ok(Loaded {
            renderer: Box::new(pollster::block_on(WindowSurface::new(window, open_device))?),
            fonts: None,
        }),
    }
}

pub fn run_on(
    adapter: Box<dyn Adapter>,
    options: RunOptions,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let launch = Launch {
        options,
        context: crate::system_context(),
        app: Box::new(app),
    };
    pollster::block_on(adapter.run(launch))
}
