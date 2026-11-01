use std::error::Error;
use std::sync::Arc;

use beui_core::app::App;
use beui_layer_xr::Link;
use beui_renderer_wgpu::present::OpenDevice;
use beui_renderer_wgpu::window::WindowSurface;
use beui_runner::renderer::{Loaded, WindowHandle};

use beui_runner::{Launch, Layer, Platform, RunOptions};

use beui_runner::environment::{Base, Choice, Stacked};

pub fn run_with(mut options: RunOptions, app: impl App + 'static) -> Result<(), Box<dyn Error>> {
    let mut others = std::mem::take(&mut options.platforms);
    let mut open_device = OpenDevice {
        extensions: options.device_extensions,
        gpu: None,
    };
    let mut natives = vec![NATIVE];
    natives.extend(others.iter().map(|platform| platform.name()));
    let choice = from_environment(&natives)?;
    let other = match &choice {
        Some(Choice {
            platform: Base::Native(index),
            ..
        }) => index.checked_sub(1),
        _ => None,
    };
    let link = match other {
        None => link(choice.as_ref(), &application_name(&options)),
        Some(index) => {
            if choice
                .as_ref()
                .is_some_and(|choice| choice.layers.contains(&Stacked::Xr))
            {
                eprintln!(
                    "beui: {} cannot show space on a headset",
                    natives[index + 1]
                );
            }
            None
        }
    };
    if let Some(link) = &link {
        open_device = link.open_device(open_device.extensions);
    }
    let app: Box<dyn App> = match (&choice, options.saved_preferences.clone()) {
        (
            Some(Choice {
                platform: Base::Native(_),
                ..
            })
            | None,
            Some(path),
        ) => Box::new(crate::saved_preferences::SavedPreferences::new(app, path)),
        _ => Box::new(app),
    };
    let (platform, layers) =
        choose_platform(choice, link, open_device.clone(), |index| {
            match index.checked_sub(1) {
                Some(other) => others.remove(other),
                None => native_platform(open_device),
            }
        })?;
    let launch = Launch {
        options,
        context: crate::native::system_context(),
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
fn from_environment(natives: &[&str]) -> Result<Option<Choice>, Box<dyn Error>> {
    let choice = beui_runner::environment::choose(natives, |name| std::env::var(name).ok())?;
    Ok(Some(choice))
}

#[cfg(target_os = "android")]
fn from_environment(_natives: &[&str]) -> Result<Option<Choice>, Box<dyn Error>> {
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
    native: impl FnOnce(usize) -> Box<dyn Platform>,
) -> Result<(Box<dyn Platform>, Vec<Box<dyn Layer>>), Box<dyn Error>> {
    let Some(choice) = choice else {
        return Ok((native(0), Vec::new()));
    };
    let platform = match choice.platform {
        Base::Native(index) => native(index),
        #[cfg(not(target_os = "android"))]
        Base::Headless {
            window,
            screen,
            pinned_clock,
        } => Box::new(beui_platform_headless::Headless {
            window,
            screen,
            open_device,
            render: choice.render,
            pinned_clock,
        }),
        #[cfg(target_os = "android")]
        Base::Headless { .. } => {
            let _ = open_device;
            native(0)
        }
    };
    let above = beui_runner::environment::drive_taps(&choice.layers);
    let mut taps: Vec<beui_layer_drive::Drive> = match above.is_empty() {
        true => Vec::new(),
        false => {
            let path = std::env::var_os(beui_runner::environment::DRIVE_SOCKET)
                .ok_or("BEUI_LAYERS names drive without BEUI_DRIVE_SOCKET")?;
            let inbox = serve(std::path::Path::new(&path))?;
            let names: Vec<Option<&'static str>> =
                above.iter().map(|layer| layer.map(Stacked::name)).collect();
            let mut taps = beui_layer_drive::Drive::taps(inbox, &names);
            taps.reverse();
            taps
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
    let served = match std::env::var(beui_runner::environment::DRIVE_LISTENER) {
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
            beui_runner::environment::DRIVE_SOCKET,
            path.display()
        )
    })?;
    Ok(inbox)
}

#[cfg(not(all(unix, not(target_os = "android"))))]
fn serve(_path: &std::path::Path) -> Result<beui_layer_drive::Inbox, Box<dyn Error>> {
    Err("BEUI_DRIVE_SOCKET needs a Unix socket, which this platform lacks".into())
}

pub fn native_platform(open_device: OpenDevice) -> Box<dyn Platform> {
    let load = move |window: Arc<dyn WindowHandle>| -> Result<Loaded, Box<dyn Error>> {
        Ok(Loaded {
            renderer: Box::new(pollster::block_on(WindowSurface::new(window, open_device))?),
            fonts: None,
        })
    };
    #[cfg(target_os = "android")]
    return Box::new(beui_platform_android::Android::new(load));
    #[cfg(not(target_os = "android"))]
    return Box::new(beui_platform_winit::Winit::new(load));
}
