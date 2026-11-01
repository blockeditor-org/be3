#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod shown;
#[cfg(windows)]
mod win32;
#[cfg(any(windows, test))]
mod win32_map;
#[cfg(any(windows, test))]
mod win32_overlay;
#[cfg(any(windows, test))]
mod win32_shape;

use std::time::Duration;

use beui::native::{Context, Document, Setup};
use beui::reactive::*;
use beui::styled::*;
use beui::{NodeId, Rect};
use block_plugin_api::{HostWindow, HostWindowId};

use crate::programs::ProgramSource;

#[cfg(target_os = "linux")]
pub use linux::{Linux as Host, set_keyboard};
#[cfg(windows)]
pub use win32::Win32 as Host;

#[cfg(not(any(target_os = "linux", windows)))]
pub struct Host;

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

    fn set_owned(_owned: bool) {}

    fn set_blank_after(_after: Option<Duration>) {}

    fn set_lock_after(_after: Option<Duration>) {}

    fn take_lock_due() -> bool {
        false
    }

    fn set_locked(_locked: bool) {}

    fn set_primary_selection(_on: bool) {}

    fn revision() -> u64 {
        0
    }

    fn listed(_hidden: &[HostWindowId]) -> Vec<HostWindow> {
        Vec::new()
    }

    fn opening_windows() -> Vec<(uuid::Uuid, HostWindowId)> {
        Vec::new()
    }

    fn close(_window: HostWindowId) {}

    fn fullscreen(_window: HostWindowId, _fullscreen: bool) {}

    fn focus(_window: HostWindowId) {}

    fn show(_window: HostWindowId) {}

    fn hide(_window: HostWindowId) {}

    fn launch(_command: String) -> bool {
        false
    }

    fn exec(_arguments: Vec<String>, _launch: Option<u64>) -> bool {
        false
    }

    #[component]
    fn WindowActions() -> NodeId {
        view! {
            <Frame />
        }
    }

    #[component]
    fn WindowSurface(window: HostWindowId) -> NodeId {
        let _ = window;
        view! {
            <Frame />
        }
    }
}
