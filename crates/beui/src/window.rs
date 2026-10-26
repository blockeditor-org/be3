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
    let (adapter, app): (Box<dyn Adapter>, Box<dyn App>) = match (
        from_environment(&mut options, open_device)?,
        options.saved_preferences.clone(),
    ) {
        (Some(headless), _) => (headless, Box::new(app)),
        (None, Some(path)) => (
            window_adapter(renderers),
            Box::new(crate::saved_preferences::SavedPreferences::new(app, path)),
        ),
        (None, None) => (window_adapter(renderers), Box::new(app)),
    };
    let launch = Launch {
        options,
        context: crate::system_context(),
        app,
        host: None,
    };
    pollster::block_on(in_space(adapter).run(launch))
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
    if let Some(size) = screen
        && !(WindowSize {
            size,
            scale: window.scale,
        })
        .fits()
    {
        return Err(format!(
            "BEUI_HEADLESS_SCREEN at scale {} is more than {} pixels across",
            window.scale,
            beui_core::app::automation::MAX_PIXELS
        )
        .into());
    }
    let render = match std::env::var("BEUI_RENDERER").as_deref() {
        Ok("none") => false,
        Ok("wgpu") | Err(_) => true,
        Ok(other) => {
            return Err(format!("BEUI_RENDERER takes wgpu or none, not {other}").into());
        }
    };
    Ok(Some(Box::new(beui_adapter_headless::Headless {
        window,
        screen,
        open_device,
        render,
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
    use beui_core::app::automation::socket;
    let inbox = beui_core::app::automation::Inbox::default();
    let served = match std::env::var("BEUI_AUTOMATION_LISTENER") {
        Ok(fd) => {
            let fd: std::os::fd::RawFd = fd
                .parse()
                .map_err(|_| format!("BEUI_AUTOMATION_LISTENER takes a file descriptor, not {fd}"))?;
            // unsafe(ffi)
            let inherited = unsafe {
                <std::os::unix::net::UnixListener as std::os::fd::FromRawFd>::from_raw_fd(fd)
            };
            inherited
                .try_clone()
                .and_then(|listener| socket::serve_on(listener, path, inbox.clone()))
        }
        Err(_) => socket::serve(path, inbox.clone()),
    };
    served.map_err(|error| {
        format!(
            "BEUI_AUTOMATION: could not listen on {}: {error}",
            path.display()
        )
    })?;
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
        host: None,
    };
    pollster::block_on(in_space(adapter).run(launch))
}

fn in_space(adapter: Box<dyn Adapter>) -> Box<dyn Adapter> {
    match std::env::var("BEUI_SPACE") {
        Ok(value) if !value.is_empty() && value != "0" => {
            Box::new(beui_core::runner::HostedAdapter {
                adapter,
                host: Box::new(beui_adapter_space::Space::new()),
            })
        }
        _ => adapter,
    }
}
