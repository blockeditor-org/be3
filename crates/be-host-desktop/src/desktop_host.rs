#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod win32;
#[cfg(any(windows, test))]
mod win32_content;
#[cfg(any(windows, test))]
mod win32_map;
#[cfg(any(windows, test))]
mod win32_overlay;
#[cfg(any(windows, test))]
mod win32_shape;

use std::time::Duration;

use be_sdk_api::{HostWindow, HostWindowId};
use beui::native::{Context, Document, Setup};
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, Rect};

use crate::programs::ProgramSource;
use crate::window_body::{WindowBody, WindowStatus};

#[cfg(target_os = "linux")]
pub use linux::{Linux as Host, screenshot, set_keyboard, set_layout};
#[cfg(windows)]
pub use win32::Win32 as Host;

#[cfg(not(any(target_os = "linux", windows)))]
pub struct Host;

/// How the host runs the desktop, set by different parts of be-host and handed to the
/// platform whole whenever one changes ([`configure`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesktopSettings {
    /// Whether a desktop shell owns the program windows: without one they get no input.
    pub owned: bool,
    /// How long the session idles before the screens blank, if ever.
    pub blank_after: Option<Duration>,
    /// How long the session idles before it locks, if ever.
    pub lock_after: Option<Duration>,
    /// Whether middle-click pastes the primary selection.
    pub primary_selection: bool,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        let display = be_block::DisplaySettings::default();
        Self {
            owned: false,
            blank_after: display.screen_off().after(),
            lock_after: display.lock_time(),
            primary_selection: false,
        }
    }
}

thread_local! {
    static SETTINGS: std::cell::Cell<DesktopSettings> = std::cell::Cell::new(DesktopSettings::default());
}

/// The desktop's settings as last configured, for a platform that starts after they were.
pub fn settings() -> DesktopSettings {
    SETTINGS.get()
}

/// Changes the desktop's settings, telling the platform when they changed.
pub fn configure(change: impl FnOnce(&mut DesktopSettings)) {
    let before = SETTINGS.get();
    let mut settings = before;
    change(&mut settings);
    if settings != before {
        SETTINGS.set(settings);
        Host::configure(&settings);
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
impl DesktopHost for Host {
    type Source = ();
}

#[component]
pub trait DesktopHost {
    type Source: ProgramSource;

    fn create() {}

    fn start(_setup: &Setup) {}

    fn environment() -> Vec<(String, String)> {
        Vec::new()
    }

    fn running() -> bool {
        false
    }

    fn before(_context: &Context, _rect: Rect, _document: &mut Document) {}

    fn after(_context: &Context, _document: &mut Document) {}

    fn frame() {}

    fn replace_gpu(_setup: &Setup) {}

    fn exiting() {}

    fn configure(_settings: &DesktopSettings) {}

    fn take_lock_due() -> bool {
        false
    }

    fn set_locked(_locked: bool) {}

    fn revision() -> u64 {
        0
    }

    fn listed(_hidden: &[HostWindowId]) -> Vec<HostWindow> {
        Vec::new()
    }

    fn opening_windows() -> Vec<(uuid::Uuid, HostWindowId)> {
        Vec::new()
    }

    fn window_status(_window: HostWindowId) -> Option<WindowStatus> {
        None
    }

    /// What `window` shows, for a host that composites program windows itself rather than in
    /// their views (be-host's `RootShell`): none until it drew, and none on a platform whose
    /// windows are not textures.
    fn picture(_window: HostWindowId) -> Option<WindowPicture> {
        None
    }

    /// The program windows a host that composites them itself shows this frame, each where it
    /// is in the host window's points: each configured to its size, as a view laid out there
    /// configures it, and every other one it showed before let go of, as a view's going does.
    fn place_windows(_placed: &[(HostWindowId, Rect)]) {}

    fn close(_window: HostWindowId) {}

    fn fullscreen(_window: HostWindowId, _fullscreen: bool) {}

    fn exec(_arguments: Vec<String>, _launch: Option<u64>) -> bool {
        false
    }

    #[component]
    fn WindowActions() -> NodeId {
        view! {
            <Frame />
        }
    }

    /// Draws `window`. `focus` fires when the shell gives the window the keyboard: it gives the
    /// surface's node beui's keyboard focus and nothing more, so on Windows it never starts focus
    /// mode, which only a click on the content does (`win32_content`).
    #[component]
    fn WindowSurface(window: HostWindowId, focus: Trigger) -> NodeId {
        let _ = (window, focus);
        view! {
            <Frame />
        }
    }
}

/// What a program window shows (`DesktopHost::picture`): its surfaces, bottom first, each a
/// texture on the host's device where it is in the window's logical pixels. The same picture
/// while the window draws nothing new (`same`).
#[derive(Clone)]
pub struct WindowPicture {
    pub surfaces: Vec<PictureSurface>,
    source: std::rc::Rc<dyn PictureSource>,
}

/// One of a window's surfaces in its picture: the part of `texture` that `uv` (from 0 to 1
/// across and down) names, drawn over `rect`, in the window's logical pixels.
#[derive(Clone)]
pub struct PictureSurface {
    pub texture: wgpu::Texture,
    pub opaque: bool,
    pub rect: Rect,
    pub uv: Rect,
    pub alpha: f32,
}

/// Where a picture came from, which hears that it was drawn.
trait PictureSource {
    fn painted(&self);
    fn pixels(&self, texture: &wgpu::Texture) -> Option<beui::native::painting::Pixels>;
    fn key(&self) -> usize;
}

impl WindowPicture {
    /// Whether it is `other`: what the window drew last both times.
    pub fn same(&self, other: &Self) -> bool {
        self.source.key() == other.source.key()
    }

    /// It was drawn, which the program hears with its next frame callbacks, as when its view
    /// draws it.
    pub fn painted(&self) {
        self.source.painted();
    }

    /// The pixels of one of its surfaces' textures, for a painting: none for one the device
    /// cannot read back.
    pub fn pixels(&self, texture: &wgpu::Texture) -> Option<beui::native::painting::Pixels> {
        self.source.pixels(texture)
    }
}

#[component]
pub fn PlacedWindow(window: HostWindowId, focus: Trigger) -> NodeId {
    let status = create_memo(move || Host::window_status(window));
    let on_force_close = Callback::new(move |()| Host::close(window));
    view! {
        <WindowBody status on_force_close>
            <Host::WindowSurface window focus />
        </WindowBody>
    }
}

#[cfg(test)]
mod tests;
