use std::error::Error;
use std::sync::Arc;

use beui_adapter_xr::Xr;
use beui_core::app::App;
use beui_core::renderer::{Loaded, WindowHandle, any_loaded};
use beui_renderer_wgpu::present::OpenDevice;
use beui_renderer_wgpu::window::WindowSurface;

use beui_core::runner::{Adapter, Launch, RunOptions};

use beui_core::runner::environment::{Base, Choice};

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
    mut options: RunOptions,
    mut renderers: Vec<WindowRenderer>,
    app: impl App + 'static,
) -> Result<(), Box<dyn Error>> {
    let choice = from_environment(&mut options, NATIVE)?;
    let xr = open_xr(choice.as_ref(), &application_name(&options));
    if let Some(xr) = &xr {
        for renderer in &mut renderers {
            let WindowRenderer::Wgpu { open_device } = renderer;
            *open_device = xr.open_device(open_device.extensions);
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
                adapter: Base::Native,
                ..
            })
            | None,
            Some(path),
        ) => Box::new(crate::saved_preferences::SavedPreferences::new(app, path)),
        _ => Box::new(app),
    };
    let adapter = adapt(choice, xr, open_device, || window_adapter(renderers));
    let launch = Launch {
        options,
        context: crate::system_context(),
        app,
        host: None,
    };
    pollster::block_on(adapter.run(launch))
}

#[cfg(target_os = "android")]
const NATIVE: &str = "android";
#[cfg(not(target_os = "android"))]
const NATIVE: &str = "winit";

#[cfg(not(target_os = "android"))]
fn from_environment(
    options: &mut RunOptions,
    native: &str,
) -> Result<Option<Choice>, Box<dyn Error>> {
    if options.automation.is_none()
        && let Some(path) = std::env::var_os(beui_core::runner::environment::AUTOMATION)
    {
        options.automation = Some(serve(std::path::Path::new(&path))?);
    }
    let choice = beui_core::runner::environment::choose(native, |name| std::env::var(name).ok())?;
    Ok(Some(choice))
}

#[cfg(target_os = "android")]
fn from_environment(
    _options: &mut RunOptions,
    _native: &str,
) -> Result<Option<Choice>, Box<dyn Error>> {
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

fn open_xr(choice: Option<&Choice>, application: &str) -> Option<Xr> {
    if !choice.is_some_and(|choice| choice.xr) {
        return None;
    }
    match Xr::open(application) {
        Ok(xr) => {
            eprintln!("beui: showing space on {}", xr.runtime());
            Some(xr)
        }
        Err(error) => {
            eprintln!("beui: no headset, so space runs as the simulator: {error}");
            None
        }
    }
}

fn adapt(
    choice: Option<Choice>,
    xr: Option<Xr>,
    open_device: OpenDevice,
    native: impl FnOnce() -> Box<dyn Adapter>,
) -> Box<dyn Adapter> {
    let Some(choice) = choice else {
        return native();
    };
    let adapter = match choice.adapter {
        Base::Native => native(),
        #[cfg(not(target_os = "android"))]
        Base::Headless { window, screen } => Box::new(beui_adapter_headless::Headless {
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
    if !choice.space {
        return adapter;
    }
    let space = beui_adapter_space::Space::new();
    let host: Box<dyn beui_core::runner::WindowHost> = match xr {
        Some(xr) => Box::new(beui_adapter_xr::XrSpace::new(space, xr.headset())),
        None => Box::new(space),
    };
    Box::new(beui_core::runner::HostedAdapter { adapter, host })
}

#[cfg(all(unix, not(target_os = "android")))]
fn serve(path: &std::path::Path) -> Result<beui_core::app::automation::Inbox, Box<dyn Error>> {
    let inbox = beui_core::app::automation::Inbox::default();
    beui_core::app::automation::socket::serve(path, inbox.clone()).map_err(|error| {
        format!(
            "{}: could not listen on {}: {error}",
            beui_core::runner::environment::AUTOMATION,
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
    let choice = from_environment(&mut options, adapter.name())?;
    if choice.as_ref().is_some_and(|choice| choice.xr) {
        eprintln!("beui: {} cannot show space on a headset", adapter.name());
    }
    let adapter = adapt(choice, None, OpenDevice::default(), || adapter);
    let launch = Launch {
        options,
        context: crate::system_context(),
        app: Box::new(app),
        host: None,
    };
    pollster::block_on(adapter.run(launch))
}
