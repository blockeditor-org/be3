use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError};

use be_wayland::programs::{DesktopEntry, Environment, IconThemes, load_icon};
use be_wayland::{Compositor, Launch, Server, WindowId, WindowView, Windows};
use beui::reactive::{Frame, component, view};
use beui::styled::LauncherItem;
use beui::{Context, Document, NodeId, Rect, Setup};
use block_plugin_api::{ChildRect, HostWindow, HostWindowId, Size};

struct Running {
    compositor: Compositor,
    cursor: Option<beui_adapter_drm::SoftwareCursor>,
    screens: Option<beui_adapter_drm::Screens>,
}

thread_local! {
    static WINDOWS: RefCell<Option<Windows>> = const { RefCell::new(None) };
    static RUNNING: RefCell<Option<Running>> = const { RefCell::new(None) };
}

pub(crate) fn create() {
    WINDOWS.with(|windows| *windows.borrow_mut() = Some(Windows::new()));
}

fn windows() -> Option<Windows> {
    WINDOWS.with(|windows| windows.borrow().clone())
}

pub(crate) fn start(setup: &Setup) {
    let (Some(windows), Some(gpu)) = (windows(), setup.get::<beui::GpuSetup>()) else {
        return;
    };
    let server = match Server::new() {
        Ok(server) => server,
        Err(error) => {
            eprintln!("block-app: the Wayland server did not start: {error}");
            crate::notices::report(format!(
                "Programs cannot be run, since the Wayland server did not start: {error}"
            ));
            return;
        }
    };
    let mut compositor = Compositor::new(server, windows);
    compositor.on_failure(crate::notices::report);
    compositor.start(
        gpu.device.clone(),
        gpu.queue.clone(),
        gpu.format,
        setup.waker.clone(),
    );
    let cursor = setup.get::<beui_adapter_drm::SoftwareCursor>().cloned();
    let screens = setup.get::<beui_adapter_drm::Screens>().cloned();
    RUNNING.with(|running| {
        *running.borrow_mut() = Some(Running {
            compositor,
            cursor,
            screens,
        });
    });
}

fn with<R>(act: impl FnOnce(&mut Running) -> R) -> Option<R> {
    RUNNING.with(|running| running.borrow_mut().as_mut().map(act))
}

pub(crate) fn running() -> bool {
    with(|_| ()).is_some()
}

pub(crate) fn before(context: &Context, rect: Rect, document: &mut Document) {
    with(|running| {
        if let Some(screens) = &running.screens {
            running.compositor.set_screens(screens.rects());
        }
        running.compositor.before(context, rect, document);
    });
}

pub(crate) fn after(context: &Context, document: &mut Document) {
    with(|running| {
        running.compositor.after(context, document);
        if let Some(cursor) = &running.cursor {
            cursor.set(running.compositor.cursor_image().map(|image| {
                beui_adapter_drm::CursorImage {
                    texture: image.texture,
                    size: image.size,
                    hotspot: image.hotspot,
                    opaque: image.opaque,
                }
            }));
        }
    });
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
        .map(|info| HostWindow {
            id: HostWindowId(info.id.0),
            title: info.title,
            app_id: info.app_id,
            parent: info.parent.map(|parent| HostWindowId(parent.0)),
            size: Size {
                width: info.size.x,
                height: info.size.y,
            },
            fullscreen: info.fullscreen.map(|area| ChildRect {
                x: area.min.x,
                y: area.min.y,
                width: area.width(),
                height: area.height(),
            }),
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

pub(crate) fn launch(command: String) -> bool {
    let Some(windows) = windows().filter(|_| running()) else {
        return false;
    };
    windows.launch(command);
    crate::host::wake();
    true
}

enum Scanned {
    Entries(Environment, Vec<DesktopEntry>),
    Icons(Vec<(String, beui::Image)>),
}

#[derive(Default)]
pub(crate) struct Programs {
    environment: Environment,
    entries: Vec<DesktopEntry>,
    icons: HashMap<String, beui::Image>,
    items: Rc<Vec<LauncherItem>>,
    scanning: Option<Receiver<Scanned>>,
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
                        Some((id, load_icon(&path, pixels)?))
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
        if changed {
            self.items = Rc::new(self.entries.iter().map(|entry| self.item(entry)).collect());
        }
        changed
    }

    fn item(&self, entry: &DesktopEntry) -> LauncherItem {
        let detail = match entry.comment.is_empty() {
            true => entry.generic_name.clone(),
            false => entry.comment.clone(),
        };
        let id = entry.id.trim_end_matches(".desktop").to_owned();
        LauncherItem {
            key: entry.id.clone(),
            title: entry.name.clone(),
            detail,
            terms: std::iter::once(entry.generic_name.clone())
                .chain(entry.keywords.iter().cloned())
                .chain(std::iter::once(id))
                .collect(),
            image: self.icons.get(&entry.id).cloned(),
        }
    }

    pub(crate) fn items(&self) -> Rc<Vec<LauncherItem>> {
        Rc::clone(&self.items)
    }

    pub(crate) fn launch(&self, key: &str) -> bool {
        let Some(entry) = self.entries.iter().find(|entry| entry.id == key) else {
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
        });
        crate::host::wake();
        true
    }
}

#[component]
pub(crate) fn WindowSurface(window: HostWindowId) -> NodeId {
    match windows() {
        Some(windows) => view! {
            <WindowView windows id={WindowId(window.0)} />
        },
        None => view! {
            <Frame />
        },
    }
}
