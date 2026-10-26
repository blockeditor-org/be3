pub mod command_line;
pub mod cursor;
pub mod events;
pub mod filter;
pub mod focus;
pub mod hotkey;
pub mod icon;
pub mod integrity;
pub mod launches;
pub mod model;
pub mod monitors;
pub mod power;
pub mod programs;

#[cfg(windows)]
mod win32;

pub use cursor::{Cursor, Over};
pub use events::Event;
pub use focus::{
    Command, EndReason, FocusError, FocusEvent, FocusMode, Focused, Foreground, Output,
};
pub use hotkey::{Hotkey, HotkeyError, Modifiers};
pub use integrity::{Control, InputDesktop, IntegrityLevel};
pub use launches::{Launch, Launches, Origin};
pub use model::{Entry, EntryId, EntryKind, Model, Popup, Snapshot};
pub use monitors::Monitor;
pub use power::{PowerAction, PowerAvailability};
pub use programs::{Program, Shortcut};
#[cfg(windows)]
pub use win32::{
    Watcher, Win32Desktop, bring_forward, close_window, exec, launch, power_availability,
    program_icon, request_power, start_menu_programs,
};

use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Handle(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

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

    pub fn origin(&self) -> Point {
        Point::new(self.left, self.top)
    }

    pub fn centre(&self) -> Point {
        Point::new(self.left + self.width() / 2, self.top + self.height() / 2)
    }

    pub fn union(&self, other: &Rect) -> Rect {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        Rect::new(
            self.left.min(other.left),
            self.top.min(other.top),
            self.right.max(other.right),
            self.bottom.max(other.bottom),
        )
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let rect = Rect::new(
            self.left.max(other.left),
            self.top.max(other.top),
            self.right.min(other.right),
            self.bottom.min(other.bottom),
        );
        (!rect.is_empty()).then_some(rect)
    }

    pub fn clamp(&self, point: Point) -> Point {
        Point::new(
            point.x.clamp(self.left, (self.right - 1).max(self.left)),
            point.y.clamp(self.top, (self.bottom - 1).max(self.top)),
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    pub popup: bool,
    pub child: bool,
    pub tool_window: bool,
    pub app_window: bool,
    pub no_activate: bool,
    pub dialog_frame: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawWindow {
    pub handle: Handle,
    pub owner: Option<Handle>,
    pub process_id: u32,
    pub thread_id: u32,
    pub class: String,
    pub title: String,
    pub visible: bool,
    pub cloaked: bool,
    pub enabled: bool,
    pub minimized: bool,
    pub style: Style,
    pub frame: Rect,
    pub responding: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Process {
    pub exe: Option<PathBuf>,
    pub integrity: Option<IntegrityLevel>,
}

pub trait Desktop {
    fn windows(&self) -> Vec<RawWindow>;
    fn process(&self, process_id: u32) -> Process;
    fn icon(&self, window: Handle) -> Option<Arc<Image>>;
    fn app_user_model_id(&self, window: Handle) -> Option<String>;
    fn monitors(&self) -> Vec<Monitor>;
    fn foreground(&self) -> Option<Handle>;
    fn input_desktop(&self) -> InputDesktop;
    fn own_process_id(&self) -> u32;
    fn own_integrity(&self) -> Option<IntegrityLevel>;
}

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests;
