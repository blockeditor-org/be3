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
    mut options: RunOptions,
    renderers: Vec<WindowRenderer>,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let open_device = renderers.iter().find_map(|renderer| match renderer {
        WindowRenderer::Wgpu { open_device } => open_device.clone(),
    });
    let adapter = match from_environment(&mut options, open_device)? {
        Some(headless) => headless,
        None => window_adapter(renderers),
    };
    let launch = Launch {
        options,
        context: crate::system_context(),
        app: Box::new(app),
    };
    pollster::block_on(adapter.run(launch))
}

#[cfg(not(target_os = "android"))]
fn from_environment(
    options: &mut RunOptions,
    open_device: Option<OpenDevice>,
) -> Result<Option<Box<dyn Adapter>>, Box<dyn Error>> {
    use beui_core::app::automation::WindowSize;
    if options.automation.is_none()
        && let Some(path) = std::env::var_os("BEUI_AUTOMATION")
    {
        options.automation = Some(serve(std::path::Path::new(&path))?);
    }
    let Ok(window) = std::env::var("BEUI_HEADLESS") else {
        return Ok(None);
    };
    let window = WindowSize::parse(&window)
        .ok_or_else(|| format!("BEUI_HEADLESS takes WIDTHxHEIGHT[@SCALE], not {window}"))?;
    let screen = match std::env::var("BEUI_HEADLESS_SCREEN") {
        Ok(screen) => Some(
            WindowSize::parse(&screen)
                .ok_or_else(|| format!("BEUI_HEADLESS_SCREEN takes WIDTHxHEIGHT, not {screen}"))?
                .size,
        ),
        Err(_) => None,
    };
    Ok(Some(Box::new(beui_adapter_headless::Headless {
        window,
        screen,
        open_device,
    })))
}

#[cfg(target_os = "android")]
fn from_environment(
    _options: &mut RunOptions,
    _open_device: Option<OpenDevice>,
) -> Result<Option<Box<dyn Adapter>>, Box<dyn Error>> {
    Ok(None)
}

#[cfg(all(unix, not(target_os = "android")))]
fn serve(path: &std::path::Path) -> Result<beui_core::app::automation::Inbox, Box<dyn Error>> {
    let inbox = beui_core::app::automation::Inbox::default();
    beui_core::app::automation::socket::serve(path, inbox.clone())
        .map_err(|error| format!("BEUI_AUTOMATION: could not listen on {}: {error}", path.display()))?;
    Ok(inbox)
}

#[cfg(not(any(unix, target_os = "android")))]
fn serve(_path: &std::path::Path) -> Result<beui_core::app::automation::Inbox, Box<dyn Error>> {
    Err("BEUI_AUTOMATION needs a Unix socket, which this platform lacks".into())
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
    mut options: RunOptions,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let adapter = from_environment(&mut options, None)?.unwrap_or(adapter);
    let launch = Launch {
        options,
        context: crate::system_context(),
        app: Box::new(app),
    };
    pollster::block_on(adapter.run(launch))
}
