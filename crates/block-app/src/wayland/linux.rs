use std::cell::RefCell;

use be_wayland::{Compositor, FullscreenWindow, Server, WindowId, WindowView, Windows};
use beui::reactive::{Frame, component, view};
use beui::{Context, Document, NodeId, Rect, Setup};
use block_plugin_api::{HostWindow, HostWindowId, Size};

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
            return;
        }
    };
    let mut compositor = Compositor::new(server, windows);
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
        })
        .collect()
}

pub(crate) fn close(window: HostWindowId) {
    if let Some(windows) = windows() {
        windows.close(WindowId(window.0));
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

#[component]
pub(crate) fn FullscreenSurface() -> NodeId {
    match windows() {
        Some(windows) => view! {
            <FullscreenWindow windows />
        },
        None => view! {
            <Frame />
        },
    }
}
