use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use super::DesktopHost;
use super::shown::Shown;
use crate::programs::{Opener, ProgramSource};
use be_wayland::programs::{DesktopEntry, Environment, IconThemes, MimeGlobs, load_icon};
use be_wayland::{Compositor, Launch, Server, WindowId, WindowView, Windows};
use beui::native::Document;
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, Rect, native::Context, native::Setup};
use block_plugin_api::{
    HostImage, HostProgram, HostWindow, HostWindowId, WindowKind, WindowOrigin,
};

struct Running {
    compositor: Compositor,
    cursor: Option<beui_platform_drm::SoftwareCursor>,
    display: Option<beui_platform_drm::DisplayControl>,
    flips: Option<beui_platform_drm::Flips>,
    displays: Option<beui_platform_drm::Displays>,
    blanked: bool,
    lock_due: bool,
    lock_noticed: bool,
}

#[derive(Clone, Default)]
struct Launched {
    program: Option<String>,
    name: String,
    launch: Option<u64>,
    block: Option<uuid::Uuid>,
}

thread_local! {
    static WINDOWS: Cell<Option<Windows>> = const { Cell::new(None) };
    static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
    static LAUNCHES: RefCell<Vec<Launched>> = const { RefCell::new(Vec::new()) };
    static SHOWN: RefCell<Shown> = RefCell::new(Shown::default());
    static NAMES: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

fn tag(
    program: Option<&DesktopEntry>,
    launch: Option<u64>,
    block: Option<uuid::Uuid>,
) -> Option<u64> {
    if program.is_none() && launch.is_none() {
        return None;
    }
    LAUNCHES.with(|launches| {
        let mut launches = launches.borrow_mut();
        launches.push(Launched {
            program: program.map(|entry| entry.id.clone()),
            name: program.map(|entry| entry.name.clone()).unwrap_or_default(),
            launch,
            block,
        });
        u64::try_from(launches.len() - 1).ok()
    })
}

fn launched(tag: Option<u64>) -> Launched {
    let index = tag.and_then(|tag| usize::try_from(tag).ok());
    LAUNCHES.with(|launches| {
        index
            .and_then(|index| launches.borrow().get(index).cloned())
            .unwrap_or_default()
    })
}

pub struct Linux;

#[component]
impl DesktopHost for Linux {
    type Source = LinuxPrograms;

    fn create() {
        let windows = Windows::new();
        WINDOWS.set(Some(windows));
    }

    fn start(setup: &Setup) {
        let (Some(windows), Some(gpu)) = (windows(), setup.get::<beui_host::native::GpuSetup>())
        else {
            return;
        };
        let mut server = match Server::new() {
            Ok(server) => server,
            Err(error) => {
                eprintln!("block-app: the Wayland server did not start: {error}");
                block_app_base::notices::report(format!(
                    "Programs cannot be run, since the Wayland server did not start: {error}"
                ));
                return;
            }
        };
        match server.start_xwayland() {
            Ok(display) => eprintln!("block-app: X11 programs run on Xwayland at :{display}"),
            Err(error) => {
                eprintln!("block-app: Xwayland did not start, so X11 programs cannot run: {error}")
            }
        }
        let mut compositor = Compositor::new(server, windows);
        compositor.on_failure(block_app_base::notices::report);
        let defaults = be_block::DisplaySettings::default();
        compositor.set_blank_after(defaults.screen_off().after());
        compositor.set_lock_after(defaults.lock_time());
        compositor.start(
            gpu.device.clone(),
            gpu.queue.clone(),
            gpu.format,
            setup.waker.clone(),
        );
        let cursor = setup.get::<beui_platform_drm::SoftwareCursor>().cloned();
        let display = setup.get::<beui_platform_drm::DisplayControl>().cloned();
        let flips = setup.get::<beui_platform_drm::Flips>().cloned();
        if flips.is_some() {
            compositor.pace_to_flips();
        }
        if let Some(beui_platform_drm::Loop(handle)) = setup.get::<beui_platform_drm::Loop>() {
            compositor.join_loop(handle);
        }
        let displays = setup.get::<beui_platform_drm::Displays>().cloned();
        RUNNING.with(|running| {
            *running.borrow_mut() = Some(Running {
                compositor,
                cursor,
                display,
                flips,
                displays,
                blanked: false,
                lock_due: false,
                lock_noticed: false,
            });
        });
    }

    fn environment() -> Vec<(String, String)> {
        with(|running| {
            running
                .compositor
                .environment()
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value))
                .collect()
        })
        .unwrap_or_default()
    }

    fn running() -> bool {
        with(|_| ()).is_some()
    }

    fn before(context: &Context, rect: Rect, _document: &mut Document) {
        with(|running| {
            let monitors: Vec<be_wayland::Monitor> = running
                .displays
                .as_ref()
                .map(|displays| {
                    displays
                        .displays()
                        .into_iter()
                        .map(|display| be_wayland::Monitor {
                            rect: display.rect(),
                            id: display.id,
                            name: display.name,
                            scale: display.scale,
                        })
                        .collect()
                })
                .unwrap_or_default();
            if running
                .display
                .as_ref()
                .is_some_and(beui_platform_drm::DisplayControl::take_woken)
            {
                running.compositor.woke();
            }
            if let Some(flips) = &running.flips {
                for flip in flips.take() {
                    let Some(monitor) = monitors.iter().find(|monitor| monitor.id == flip.screen)
                    else {
                        continue;
                    };
                    running.compositor.presented(
                        monitor.rect,
                        be_wayland::Presented {
                            time: flip.time,
                            refresh: flip.refresh,
                            sequence: flip.sequence,
                            vsync: true,
                            hardware_clock: flip.hardware_clock,
                        },
                    );
                }
            }
            running.compositor.set_monitors(monitors);
            running.compositor.before(context, rect);
        });
    }

    fn after(context: &Context, document: &mut Document) {
        with(|running| {
            running.compositor.after(context, document);
            crate::remap::focus(running.compositor.keyboard_window());
            if let Some(flips) = &running.flips {
                flips.watch(running.compositor.awaits_flip());
            }
            let due = running.compositor.lock_due();
            if due && !running.lock_due {
                running.lock_noticed = true;
            }
            running.lock_due = due;
            let blanked = running.compositor.idle();
            if let Some(display) = &running.display
                && blanked != running.blanked
            {
                running.blanked = blanked;
                display.set_blanked(blanked);
            }
            if let Some(cursor) = &running.cursor {
                cursor.set(running.compositor.cursor_image().map(|image| {
                    beui_platform_drm::CursorImage {
                        texture: image.texture,
                        size: image.size,
                        hotspot: image.hotspot,
                        opaque: image.opaque,
                        revision: image.revision,
                    }
                }));
            }
        });
    }

    fn set_blank_after(after: Option<Duration>) {
        with(|running| running.compositor.set_blank_after(after));
    }

    fn set_lock_after(after: Option<Duration>) {
        with(|running| running.compositor.set_lock_after(after));
    }

    fn take_lock_due() -> bool {
        with(|running| std::mem::take(&mut running.lock_noticed)).unwrap_or(false)
    }

    fn set_locked(locked: bool) {
        with(|running| running.compositor.set_locked(locked));
    }

    fn set_owned(owned: bool) {
        with(|running| running.compositor.set_owned(owned));
    }

    fn set_primary_selection(on: bool) {
        with(|running| running.compositor.set_primary_selection(on));
    }

    fn replace_gpu(setup: &Setup) {
        let Some(gpu) = setup.get::<beui_host::native::GpuSetup>() else {
            return;
        };
        with(|running| {
            running
                .compositor
                .replace_gpu(gpu.device.clone(), gpu.queue.clone(), gpu.format);
        });
    }

    fn exiting() {
        with(|running| running.compositor.exiting());
        let running = RUNNING.with(|running| running.borrow_mut().take());
        drop(running);
        WINDOWS.set(None);
    }

    fn revision() -> u64 {
        windows().map_or(0, |windows| windows.revision())
            + SHOWN.with(|kept| kept.borrow().changes())
    }

    fn show(window: HostWindowId) {
        set_shown(window, true);
    }

    fn hide(window: HostWindowId) {
        set_shown(window, false);
    }

    fn listed(hidden: &[HostWindowId]) -> Vec<HostWindow> {
        let Some(windows) = windows() else {
            return Vec::new();
        };
        let listed = windows.list().get_untracked();
        let opening: Vec<WindowId> = hidden.iter().map(|window| WindowId(window.0)).collect();
        let ids: Vec<HostWindowId> = listed.iter().map(|info| HostWindowId(info.id.0)).collect();
        SHOWN.with(|kept| kept.borrow_mut().keep_only(&ids));
        listed
            .into_iter()
            .filter(|info| !opening.contains(&info.id))
            .map(|info| {
                let launched = launched(info.launch);
                HostWindow {
                    id: HostWindowId(info.id.0),
                    kind: WindowKind::Window,
                    shown: SHOWN.with(|kept| kept.borrow().shown(HostWindowId(info.id.0))),
                    name: match launched.name.is_empty() {
                        true => program_name(&info.app_id),
                        false => launched.name.clone(),
                    },
                    title: info.title,
                    app_id: info.app_id,
                    parent: info
                        .parent
                        .filter(|parent| !opening.contains(parent))
                        .map(|parent| HostWindowId(parent.0)),
                    modal: info.modal,
                    size: info.size,
                    fullscreen: info.fullscreen,
                    responding: info.responding,
                    focused: windows.focused() == Some(info.id),
                    icon: info.icon.map(|icon| HostImage {
                        width: icon.width(),
                        height: icon.height(),
                        rgba: icon.pixels().to_vec(),
                    }),
                    raised: windows.raised(info.id),
                    origin: WindowOrigin {
                        command: relaunch_command(&launched.program, info.command),
                        program: launched.program,
                        launch: launched.launch,
                    },
                    note: None,
                }
            })
            .collect()
    }

    fn opening_windows() -> Vec<(uuid::Uuid, HostWindowId)> {
        let Some(windows) = windows() else {
            return Vec::new();
        };
        windows
            .list()
            .get_untracked()
            .into_iter()
            .filter(|info| info.parent.is_none())
            .filter_map(|info| Some((launched(info.launch).block?, HostWindowId(info.id.0))))
            .collect()
    }

    fn close(window: HostWindowId) {
        if let Some(windows) = windows() {
            let id = WindowId(window.0);
            windows.close(id);
            if let Some(dialog) = windows.blocked_by(id) {
                windows.activate(dialog);
                block_app_base::host::wake();
            }
        }
    }

    fn fullscreen(window: HostWindowId, fullscreen: bool) {
        if let Some(windows) = windows() {
            windows.request_fullscreen(WindowId(window.0), fullscreen);
            block_app_base::host::wake();
        }
    }

    fn focus(window: HostWindowId) {
        if let Some(windows) = windows() {
            windows.activate(WindowId(window.0));
            block_app_base::host::wake();
        }
    }

    fn launch(command: String) -> bool {
        let Some(windows) = windows().filter(|_| Linux::running()) else {
            return false;
        };
        windows.launch(command);
        block_app_base::host::wake();
        true
    }

    fn exec(arguments: Vec<String>, launch: Option<u64>) -> bool {
        let Some(windows) = windows().filter(|_| Linux::running()) else {
            return false;
        };
        if arguments.is_empty() {
            return false;
        }
        windows.run(Launch {
            arguments,
            working_dir: None,
            tag: tag(None, launch, None),
        });
        block_app_base::host::wake();
        true
    }

    #[component]
    fn WindowActions() -> NodeId {
        if let Some(windows) = windows() {
            be_wayland::register_restore_shortcuts(windows);
        }
        view! {
            <Frame />
        }
    }

    #[component]
    fn WindowSurface(window: HostWindowId) -> NodeId {
        let occluders = crate::occlusion::occlusion();
        let occluders = create_memo(move || {
            occluders
                .as_ref()
                .map(|occluders| occluders.get())
                .unwrap_or_default()
        });
        match windows() {
            Some(windows) => view! {
                <WindowView windows id={WindowId(window.0)} occluders />
            },
            None => view! {
                <Frame />
            },
        }
    }
}

fn windows() -> Option<Windows> {
    WINDOWS.get()
}

fn with<R>(act: impl FnOnce(&mut Running) -> R) -> Option<R> {
    RUNNING.with(|running| running.borrow_mut().as_mut().map(act))
}

pub fn set_keyboard(keyboard: &be_wayland::KeyboardConfig) -> bool {
    with(|running| running.compositor.set_keyboard(keyboard)).unwrap_or(true)
}

fn set_shown(window: HostWindowId, shown: bool) {
    if SHOWN.with(|kept| kept.borrow_mut().set(window, shown)) {
        block_app_base::host::wake();
    }
}

fn program_name(app_id: &str) -> String {
    NAMES.with(|names| {
        names
            .borrow_mut()
            .entry(app_id.to_owned())
            .or_insert_with(|| {
                let environment = Environment::from_env();
                environment
                    .entry(app_id)
                    .or_else(|| environment.entry(&app_id.to_lowercase()))
                    .map(|entry| entry.name)
                    .unwrap_or_default()
            })
            .clone()
    })
}

fn relaunch_command(program: &Option<String>, command: Vec<String>) -> Vec<String> {
    match program {
        Some(_) => Vec::new(),
        None => command.into_iter().take(1).collect(),
    }
}

pub struct LinuxPrograms {
    environment: Environment,
    entries: Vec<DesktopEntry>,
    globs: MimeGlobs,
    themes: OnceLock<IconThemes>,
}

impl ProgramSource for LinuxPrograms {
    fn list() -> Self {
        let environment = Environment::from_env();
        Self {
            entries: environment.programs(),
            globs: environment.mime_globs(),
            environment,
            themes: OnceLock::new(),
        }
    }

    fn programs(&self) -> Vec<HostProgram> {
        self.entries
            .iter()
            .map(|entry| HostProgram {
                id: entry.id.clone(),
                name: entry.name.clone(),
                generic_name: entry.generic_name.clone(),
                comment: entry.comment.clone(),
                keywords: entry.keywords.clone(),
                wm_class: entry.startup_wm_class.clone(),
                icon: None,
            })
            .collect()
    }

    fn icon(&self, id: &str, pixels: u32) -> Option<HostImage> {
        let name = self
            .entries
            .iter()
            .find(|entry| entry.id == id)?
            .icon
            .as_ref()?;
        let themes = self
            .themes
            .get_or_init(|| IconThemes::new(&self.environment));
        let icon = load_icon(&themes.find(name, pixels)?, pixels)?;
        Some(HostImage {
            width: icon.width(),
            height: icon.height(),
            rgba: icon.pixels().to_vec(),
        })
    }

    fn openers(&self) -> Vec<Opener> {
        self.entries
            .iter()
            .filter(|entry| !entry.mime_types.is_empty())
            .map(|entry| Opener {
                id: entry.id.clone(),
                name: entry.name.clone(),
                extensions: self.globs.extensions(&entry.mime_types),
            })
            .filter(|opener| !opener.extensions.is_empty())
            .collect()
    }

    fn run(
        &self,
        id: &str,
        launch: Option<u64>,
        file: Option<(&Path, uuid::Uuid)>,
    ) -> Option<Result<(), String>> {
        let entry = self.entries.iter().find(|entry| entry.id == id)?;
        let path = file.map(|(path, _)| path.to_string_lossy().into_owned());
        let arguments = match self.environment.command(entry, path.as_deref()) {
            Ok(arguments) => arguments,
            Err(error) => {
                eprintln!("block-app: {} did not start: {error}", entry.name);
                let problem = format!("{} did not start: {error}", entry.name);
                block_app_base::notices::report(problem.clone());
                return Some(Err(problem));
            }
        };
        let Some(windows) = windows().filter(|_| Linux::running()) else {
            return Some(Err("Programs cannot run here.".to_owned()));
        };
        windows.run(Launch {
            arguments,
            working_dir: entry.working_dir.clone(),
            tag: tag(Some(entry), launch, file.map(|(_, block)| block)),
        });
        block_app_base::host::wake();
        Some(Ok(()))
    }
}

#[cfg(test)]
mod tests;
