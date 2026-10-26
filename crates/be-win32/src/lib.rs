//! Windows windows and the session, for block-app's Windows host: everything
//! about them but capture and input.
//!
//! The logic is plain Rust over the [`Desktop`] trait, so it builds and is
//! tested everywhere: the Alt-Tab-like [`filter`], the [`Model`] that turns
//! the desktop's windows and monitors into entries with stable ids and a
//! revision, popup association, launch matching, the order of the Start
//! Menu's programs, integrity levels and the secure desktop. The Win32 calls
//! are in `win32`, built only for Windows: the [`Watcher`] thread that hooks
//! window events and publishes [`Snapshot`]s, the program list and launches,
//! and the power actions.

pub mod command_line;
pub mod events;
pub mod filter;
pub mod icon;
pub mod integrity;
pub mod launches;
pub mod model;
pub mod monitors;
pub mod power;
pub mod programs;

#[cfg(windows)]
mod win32;

pub use events::Event;
pub use integrity::{Control, InputDesktop, IntegrityLevel};
pub use launches::{Launch, Launches, Origin};
pub use model::{Entry, EntryId, EntryKind, Model, Popup, Snapshot};
pub use monitors::Monitor;
pub use power::{PowerAction, PowerAvailability};
pub use programs::{Program, Shortcut};
#[cfg(windows)]
pub use win32::{
    Watcher, close_window, exec, launch, power_availability, programs, request_power,
};

use std::path::PathBuf;
use std::sync::Arc;

/// An `HWND` or an `HMONITOR`, as a number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Handle(pub u64);

/// A rectangle of the virtual desktop, in physical pixels; `right` and
/// `bottom` are exclusive.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn new(left: i32, top: i32, right: i32, bottom: i32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn width(&self) -> i32 {
        (self.right - self.left).max(0)
    }

    pub fn height(&self) -> i32 {
        (self.bottom - self.top).max(0)
    }

    pub fn is_empty(&self) -> bool {
        self.width() == 0 || self.height() == 0
    }

    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// An image as rows of rgba pixels, top row first, not premultiplied.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// The extended window styles the filter reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    /// `WS_POPUP`.
    pub popup: bool,
    /// `WS_CHILD`.
    pub child: bool,
    /// `WS_EX_TOOLWINDOW`.
    pub tool_window: bool,
    /// `WS_EX_APPWINDOW`.
    pub app_window: bool,
    /// `WS_EX_NOACTIVATE`.
    pub no_activate: bool,
    /// `WS_EX_DLGMODALFRAME`.
    pub dialog_frame: bool,
}

/// What the desktop says about one top-level window: what is cheap to read
/// every time anything changes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawWindow {
    pub handle: Handle,
    /// `GetWindow(GW_OWNER)`.
    pub owner: Option<Handle>,
    pub process_id: u32,
    pub thread_id: u32,
    pub class: String,
    pub title: String,
    /// `IsWindowVisible`.
    pub visible: bool,
    /// `DWMWA_CLOAKED`: on another virtual desktop, a suspended app, or
    /// hidden by the shell.
    pub cloaked: bool,
    /// `IsWindowEnabled`: false while a modal window it owns is open.
    pub enabled: bool,
    /// `IsIconic`.
    pub minimized: bool,
    pub style: Style,
    /// `DWMWA_EXTENDED_FRAME_BOUNDS`: the visible frame, without the
    /// invisible resize borders. A minimized window's is where Windows parks it.
    pub frame: Rect,
    /// Not `IsHungAppWindow`.
    pub responding: bool,
}

/// What the desktop says about a process, read once for each process.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Process {
    pub exe: Option<PathBuf>,
    /// `None` when its token cannot be read, which for a process of the same
    /// user means it runs elevated.
    pub integrity: Option<IntegrityLevel>,
}

/// What the logic needs from Windows. `win32` implements it with the real
/// calls, tests with a fake.
pub trait Desktop {
    /// Every top-level window, the top of the z-order first.
    fn windows(&self) -> Vec<RawWindow>;
    fn process(&self, process_id: u32) -> Process;
    /// The window's big icon, else its small one, else its class's.
    fn icon(&self, window: Handle) -> Option<Arc<Image>>;
    /// The AppUserModelID the window or its process set.
    fn app_user_model_id(&self, window: Handle) -> Option<String>;
    fn monitors(&self) -> Vec<Monitor>;
    fn foreground(&self) -> Option<Handle>;
    fn input_desktop(&self) -> InputDesktop;
    /// This process: its windows are never entries.
    fn own_process_id(&self) -> u32;
    fn own_integrity(&self) -> Option<IntegrityLevel>;
}

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests;
