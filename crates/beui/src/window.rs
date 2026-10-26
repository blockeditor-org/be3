use std::error::Error;
use std::sync::Arc;

use beui_core::app::App;
use beui_core::renderer::{Loaded, WindowHandle, any_loaded};
use beui_layer_xr::Link;
use beui_renderer_wgpu::present::OpenDevice;
use beui_renderer_wgpu::window::WindowSurface;

use beui_core::runner::{Launch, Layer, Platform, RunOptions};

use beui_core::runner::environment::{Base, Choice, Stacked};

pub enum WindowRenderer {
    Wgpu { open_device: OpenDevice },
}

impl WindowRenderer {
    pub fn wgpu() -> Self {
        Self::Wgpu {
            open_device: OpenDevice::default(),
        }
    }
}

pub fn run_with(options: RunOptions, app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    run_with_renderers(options, vec![WindowRenderer::wgpu()], app)
}

pub fn run_with_renderers(
    options: RunOptions,
    mut renderers: Vec<WindowRenderer>,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let choice = from_environment(NATIVE)?;
    let link = link(choice.as_ref(), &application_name(&options));
    if let Some(link) = &link {
        for renderer in &mut renderers {
            let WindowRenderer::Wgpu { open_device } = renderer;
            *open_device = link.open_device(open_device.extensions);
        }
    }
    let open_device = renderers
        .iter()
        .map(|renderer| match renderer {
            WindowRenderer::Wgpu { open_device } => open_device.clone(),
        })
        .next()
        .unwrap_or_default();
    let app: Box<dyn App> = match (&choice, options.saved_preferences.clone()) {
        (
            Some(Choice {
                platform: Base::Native,
                ..
            })
            | None,
            Some(path),
        ) => Box::new(crate::saved_preferences::SavedPreferences::new(app, path)),
        _ => Box::new(app),
    };
    let (platform, layers) =
        choose_platform(choice, link, open_device, || native_platform(renderers))?;
    let launch = Launch {
        options,
        context: crate::system_context(),
        app,
        layers,
    };
    pollster::block_on(platform.run(launch))
}

#[cfg(target_os = "android")]
const NATIVE: &str = "android";
#[cfg(not(target_os = "android"))]
const NATIVE: &str = "winit";

#[cfg(not(target_os = "android"))]
fn from_environment(native: &str) -> Result<Option<Choice>, Box<dyn Error>> {
    let choice = beui_core::runner::environment::choose(native, |name| std::env::var(name).ok())?;
    Ok(Some(choice))
}

#[cfg(target_os = "android")]
fn from_environment(_native: &str) -> Result<Option<Choice>, Box<dyn Error>> {
    Ok(None)
}

fn application_name(options: &RunOptions) -> String {
    options
        .app_id
        .clone()
        .or_else(|| {
            let program = std::env::current_exe().ok()?;
            Some(program.file_stem()?.to_str()?.to_owned())
        })
        .unwrap_or_else(|| "beui".to_owned())
}

fn link(choice: Option<&Choice>, application: &str) -> Option<Link> {
    choice
        .is_some_and(|choice| choice.layers.contains(&Stacked::Xr))
        .then(|| Link::new(application))
}

fn choose_platform(
    choice: Option<Choice>,
    link: Option<Link>,
    open_device: OpenDevice,
    native: impl FnOnce() -> Box<dyn Platform>,
) -> Result<(Box<dyn Platform>, Vec<Box<dyn Layer>>), Box<dyn Error>> {
    let Some(choice) = choice else {
        return Ok((native(), Vec::new()));
    };
    let platform = match choice.platform {
        Base::Native => native(),
        #[cfg(not(target_os = "android"))]
        Base::Headless { window, screen } => Box::new(beui_platform_headless::Headless {
            window,
            screen,
            open_device,
            render: choice.render,
        }),
        #[cfg(target_os = "android")]
        Base::Headless { .. } => {
            let _ = open_device;
            native()
        }
    };
    let drives = choice
        .layers
        .iter()
        .filter(|layer| **layer == Stacked::Drive)
        .count();
    let mut taps: Vec<beui_layer_drive::Drive> = match drives {
        0 => Vec::new(),
        _ => {
            let path = std::env::var_os(beui_core::runner::environment::DRIVE_SOCKET)
                .ok_or("BEUI_LAYERS names drive without BEUI_DRIVE_SOCKET")?;
            let inbox = serve(std::path::Path::new(&path))?;
            match drives {
                1 => vec![beui_layer_drive::Drive::new(inbox)],
                _ => {
                    let (screen, app) = beui_layer_drive::Drive::taps(inbox);
                    vec![app, screen]
                }
            }
        }
    };
    let mut link = link;
    let layers = choice
        .layers
        .iter()
        .filter_map(|layer| stack(*layer, &mut link, &mut taps))
        .collect();
    Ok((platform, layers))
}

fn stack(
    layer: Stacked,
    link: &mut Option<Link>,
    taps: &mut Vec<beui_layer_drive::Drive>,
) -> Option<Box<dyn Layer>> {
    match layer {
        Stacked::Space => Some(Box::new(beui_layer_space::Space::new())),
        Stacked::Xr => {
            let space = beui_layer_space::Space::new();
            Some(match link.take() {
                Some(link) => Box::new(beui_layer_xr::XrSpace::new(space, link)),
                None => Box::new(space),
            })
        }
        Stacked::Drive => Some(Box::new(taps.pop()?)),
    }
}

#[cfg(all(unix, not(target_os = "android")))]
fn serve(path: &std::path::Path) -> Result<beui_layer_drive::Inbox, Box<dyn Error>> {
    use beui_layer_drive::socket;
    let inbox = beui_layer_drive::Inbox::default();
    let served = match std::env::var(beui_core::runner::environment::DRIVE_LISTENER) {
        Ok(fd) => {
            let fd: std::os::fd::RawFd = fd
                .parse()
                .map_err(|_| format!("BEUI_DRIVE_LISTENER takes a file descriptor, not {fd}"))?;
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
            "{}: could not listen on {}: {error}",
            beui_core::runner::environment::DRIVE_SOCKET,
            path.display()
        )
    })?;
    Ok(inbox)
}

#[cfg(not(all(unix, not(target_os = "android"))))]
fn serve(_path: &std::path::Path) -> Result<beui_layer_drive::Inbox, Box<dyn Error>> {
    Err("BEUI_DRIVE_SOCKET needs a Unix socket, which this platform lacks".into())
}

pub fn native_platform(renderers: Vec<WindowRenderer>) -> Box<dyn Platform> {
    let load = move |window: Arc<dyn WindowHandle>| {
        let results = renderers
            .into_iter()
            .map(|renderer| load(renderer, window.clone()));
        any_loaded(results, |error| {
            eprintln!("beui: a renderer did not load: {error}");
        })
    };
    #[cfg(target_os = "android")]
    return Box::new(beui_platform_android::Android::new(load));
    #[cfg(not(target_os = "android"))]
    return Box::new(beui_platform_winit::Winit::new(load));
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
    platform: Box<dyn Platform>,
    options: RunOptions,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let choice = from_environment(platform.name())?;
    if choice
        .as_ref()
        .is_some_and(|choice| choice.layers.contains(&Stacked::Xr))
    {
        eprintln!("beui: {} cannot show space on a headset", platform.name());
    }
    let (platform, layers) = choose_platform(choice, None, OpenDevice::default(), || platform)?;
    let launch = Launch {
        options,
        context: crate::system_context(),
        app: Box::new(app),
        layers,
    };
    pollster::block_on(platform.run(launch))
}
