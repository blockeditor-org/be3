use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use be_wayland::programs::{DesktopEntry, Environment, IconThemes, load_icon};
use be_wayland::{Compositor, Launch, Server, WindowId, WindowView, Windows};
use beui::reactive::*;
use beui::styled::*;
use beui::{Context, Document, NodeId, Rect, Setup};
use block_plugin_api::{HostImage, HostProgram, HostWindow, HostWindowId, WindowOrigin};

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
    launch: Option<u64>,
}

thread_local! {
    static WINDOWS: Cell<Option<Windows>> = const { Cell::new(None) };
    static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
    static LAUNCHES: RefCell<Vec<Launched>> = const { RefCell::new(Vec::new()) };
}

fn tag(program: Option<String>, launch: Option<u64>) -> Option<u64> {
    if program.is_none() && launch.is_none() {
        return None;
    }
    LAUNCHES.with(|launches| {
        let mut launches = launches.borrow_mut();
        launches.push(Launched { program, launch });
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

pub(crate) fn create() {
    let windows = Windows::new();
    WINDOWS.set(Some(windows));
}

fn windows() -> Option<Windows> {
    WINDOWS.get()
}

pub(crate) fn start(setup: &Setup) {
    let (Some(windows), Some(gpu)) = (windows(), setup.get::<beui::GpuSetup>()) else {
        return;
    };
    let mut server = match Server::new() {
        Ok(server) => server,
        Err(error) => {
            eprintln!("block-app: the Wayland server did not start: {error}");
            crate::notices::report(format!(
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
    compositor.on_failure(crate::notices::report);
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

fn with<R>(act: impl FnOnce(&mut Running) -> R) -> Option<R> {
    RUNNING.with(|running| running.borrow_mut().as_mut().map(act))
}

pub(crate) fn environment() -> Vec<(String, String)> {
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

pub(crate) fn running() -> bool {
    with(|_| ()).is_some()
}

pub(crate) fn before(context: &Context, rect: Rect, _document: &mut Document) {
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
                let Some(monitor) = monitors.iter().find(|monitor| monitor.id == flip.screen) else {
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

pub(crate) fn after(context: &Context, document: &mut Document) {
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

pub(crate) fn set_blank_after(after: Option<Duration>) {
    with(|running| running.compositor.set_blank_after(after));
}

pub(crate) fn set_lock_after(after: Option<Duration>) {
    with(|running| running.compositor.set_lock_after(after));
}

pub(crate) fn take_lock_due() -> bool {
    with(|running| std::mem::take(&mut running.lock_noticed)).unwrap_or(false)
}

pub(crate) fn set_locked(locked: bool) {
    with(|running| running.compositor.set_locked(locked));
}

pub(crate) fn set_owned(owned: bool) {
    with(|running| running.compositor.set_owned(owned));
}

pub(crate) fn set_primary_selection(on: bool) {
    with(|running| running.compositor.set_primary_selection(on));
}

pub(crate) fn set_keyboard(keyboard: &be_wayland::KeyboardConfig) -> bool {
    with(|running| running.compositor.set_keyboard(keyboard)).unwrap_or(true)
}

pub(crate) fn replace_gpu(setup: &Setup) {
    let Some(gpu) = setup.get::<beui::GpuSetup>() else {
        return;
    };
    with(|running| {
        running
            .compositor
            .replace_gpu(gpu.device.clone(), gpu.queue.clone(), gpu.format);
    });
}

pub(crate) fn exiting() {
    with(|running| running.compositor.exiting());
    let running = RUNNING.with(|running| running.borrow_mut().take());
    drop(running);
    WINDOWS.set(None);
}

pub(crate) fn revision() -> u64 {
    windows().map_or(0, |windows| windows.revision())
}

pub(crate) fn listed() -> Vec<HostWindow> {
    let Some(windows) = windows() else {
        return Vec::new();
    };
    windows
        .list()
        .get_untracked()
        .into_iter()
        .map(|info| {
            let launched = launched(info.launch);
            HostWindow {
                id: HostWindowId(info.id.0),
                title: info.title,
                app_id: info.app_id,
                parent: info.parent.map(|parent| HostWindowId(parent.0)),
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
            }
        })
        .collect()
}

pub(crate) fn close(window: HostWindowId) {
    if let Some(windows) = windows() {
        windows.close(WindowId(window.0));
    }
}

pub(crate) fn fullscreen(window: HostWindowId, fullscreen: bool) {
    if let Some(windows) = windows() {
        windows.request_fullscreen(WindowId(window.0), fullscreen);
        crate::host::wake();
    }
}

pub(crate) fn focus(window: HostWindowId) {
    if let Some(windows) = windows() {
        windows.activate(WindowId(window.0));
        crate::host::wake();
    }
}

pub(crate) fn launch(command: String) -> bool {
    let Some(windows) = windows().filter(|_| running()) else {
        return false;
    };
    windows.launch(command);
    crate::host::wake();
    true
}

pub(crate) fn exec(arguments: Vec<String>, launch: Option<u64>) -> bool {
    let Some(windows) = windows().filter(|_| running()) else {
        return false;
    };
    if arguments.is_empty() {
        return false;
    }
    windows.run(Launch {
        arguments,
        working_dir: None,
        tag: tag(None, launch),
    });
    crate::host::wake();
    true
}

fn relaunch_command(program: &Option<String>, command: Vec<String>) -> Vec<String> {
    match program {
        Some(_) => Vec::new(),
        None => command.into_iter().take(1).collect(),
    }
}

const WAITING_ICON_SIZE: u32 = 64;
const ICON_BUDGET: usize = block_plugin_api::MAX_BLOB_BYTES / 2;

enum Scanned {
    Entries(Environment, Vec<DesktopEntry>),
    Icons(Vec<(String, HostImage)>),
}

#[derive(Default)]
pub(crate) struct Programs {
    environment: Environment,
    entries: Vec<DesktopEntry>,
    icons: HashMap<String, HostImage>,
    scanning: Option<Receiver<Scanned>>,
    waiting: Vec<(String, Option<u64>)>,
    scanned: bool,
}

impl Programs {
    pub(crate) fn scan(&mut self, pixels: u32) {
        if self.scanning.is_some() {
            return;
        }
        let (sender, receiver) = crate::host::waking_channel();
        self.scanning = Some(receiver);
        let spawned = std::thread::Builder::new()
            .name("programs".to_owned())
            .spawn(move || {
                let environment = Environment::from_env();
                let entries = environment.programs();
                let named: Vec<(String, String)> = entries
                    .iter()
                    .filter_map(|entry| Some((entry.id.clone(), entry.icon.clone()?)))
                    .collect();
                if sender
                    .send(Scanned::Entries(environment.clone(), entries))
                    .is_err()
                {
                    return;
                }
                let themes = IconThemes::new(&environment);
                let icons = named
                    .into_iter()
                    .filter_map(|(id, name)| {
                        let path = themes.find(&name, pixels)?;
                        let icon = load_icon(&path, pixels)?;
                        let image = HostImage {
                            width: icon.width(),
                            height: icon.height(),
                            rgba: icon.pixels().to_vec(),
                        };
                        Some((id, image))
                    })
                    .collect();
                let _ = sender.send(Scanned::Icons(icons));
            });
        if let Err(error) = spawned {
            eprintln!("block-app: the programs were not listed: {error}");
            self.scanning = None;
        }
    }

    pub(crate) fn receive(&mut self) -> bool {
        let Some(receiver) = &self.scanning else {
            return false;
        };
        let mut changed = false;
        loop {
            match receiver.try_recv() {
                Ok(Scanned::Entries(environment, entries)) => {
                    self.environment = environment;
                    self.entries = entries;
                    self.scanned = true;
                    changed = true;
                }
                Ok(Scanned::Icons(icons)) => {
                    self.icons = icons.into_iter().collect();
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.scanning = None;
                    break;
                }
            }
        }
        if self.scanned {
            for (key, launch) in std::mem::take(&mut self.waiting) {
                self.launch(&key, launch);
            }
        }
        changed
    }

    pub(crate) fn listed(&self) -> Vec<HostProgram> {
        let mut room = ICON_BUDGET;
        self.entries
            .iter()
            .map(|entry| HostProgram {
                id: entry.id.clone(),
                name: entry.name.clone(),
                generic_name: entry.generic_name.clone(),
                comment: entry.comment.clone(),
                keywords: entry.keywords.clone(),
                wm_class: entry.startup_wm_class.clone(),
                icon: self
                    .icons
                    .get(&entry.id)
                    .filter(|icon| {
                        let fits = icon.rgba.len() <= room;
                        if fits {
                            room -= icon.rgba.len();
                        }
                        fits
                    })
                    .cloned(),
            })
            .collect()
    }

    pub(crate) fn launch(&mut self, key: &str, launch: Option<u64>) -> bool {
        if !self.scanned {
            self.waiting.push((key.to_owned(), launch));
            self.scan(WAITING_ICON_SIZE);
            return false;
        }
        let Some(entry) = self.entries.iter().find(|entry| entry.id == key) else {
            eprintln!("block-app: no program {key} is installed");
            crate::notices::report(format!("{key} could not be opened: it is not installed"));
            return false;
        };
        let arguments = match self.environment.command(entry) {
            Ok(arguments) => arguments,
            Err(error) => {
                eprintln!("block-app: {} did not start: {error}", entry.name);
                crate::notices::report(format!("{} did not start: {error}", entry.name));
                return false;
            }
        };
        let Some(windows) = windows().filter(|_| running()) else {
            return false;
        };
        windows.run(Launch {
            arguments,
            working_dir: entry.working_dir.clone(),
            tag: tag(Some(entry.id.clone()), launch),
        });
        crate::host::wake();
        true
    }
}

#[component]
pub(crate) fn WindowActions() -> NodeId {
    if let Some(windows) = windows() {
        be_wayland::register_restore_shortcuts(windows);
    }
    view! {
        <Frame />
    }
}

#[component]
pub(crate) fn WindowSurface(window: HostWindowId) -> NodeId {
    let occluders = crate::compositor::region::occlusion();
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

#[cfg(test)]
mod tests;
