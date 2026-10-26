use std::error::Error;
use std::sync::Arc;

use beui_core::app::App;
use beui_core::renderer::{Loaded, WindowHandle, any_loaded};
use beui_renderer_wgpu::present::OpenDevice;
use beui_renderer_wgpu::window::WindowSurface;

use beui_core::runner::{Adapter, Launch, RunOptions};

use beui_core::runner::environment::{Base, Choice};

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
    let choice = from_environment(&mut options, NATIVE)?;
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
    let adapter = adapt(choice, open_device, || window_adapter(renderers));
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

fn adapt(
    choice: Option<Choice>,
    open_device: Option<OpenDevice>,
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
    Box::new(beui_core::runner::HostedAdapter {
        adapter,
        host: Box::new(beui_adapter_space::Space::new()),
    })
}

#[cfg(all(unix, not(target_os = "android")))]
fn serve(path: &std::path::Path) -> Result<beui_core::app::automation::Inbox, Box<dyn Error>> {
    use beui_core::app::automation::socket;
    let inbox = beui_core::app::automation::Inbox::default();
    let served = match std::env::var(beui_core::runner::environment::AUTOMATION_LISTENER) {
        Ok(fd) => {
            let fd: std::os::fd::RawFd = fd.parse().map_err(|_| {
                format!("BEUI_AUTOMATION_LISTENER takes a file descriptor, not {fd}")
            })?;
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
    let adapter = adapt(choice, None, || adapter);
    let launch = Launch {
        options,
        context: crate::system_context(),
        app: Box::new(app),
        host: None,
    };
    pollster::block_on(adapter.run(launch))
}
