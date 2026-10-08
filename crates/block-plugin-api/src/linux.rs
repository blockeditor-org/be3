use serde::{Deserialize, Serialize};

use crate::{ChildRect, Direction, Size};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HostWindowId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostWindow {
    pub id: HostWindowId,
    pub title: String,
    pub app_id: String,
    pub parent: Option<HostWindowId>,
    pub size: Size,
    pub fullscreen: Option<ChildRect>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostInputDevice {
    pub name: String,
    pub vendor: u32,
    pub product: u32,
    pub speed: Option<f64>,
    pub tap_to_click: Option<bool>,
    pub natural_scroll: Option<bool>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HostDisplayMode {
    pub width: u32,
    pub height: u32,
    pub refresh_millihertz: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostDisplay {
    pub id: String,
    pub name: String,
    pub connector: String,
    pub modes: Vec<HostDisplayMode>,
    pub default: HostDisplayMode,
    pub current: HostDisplayMode,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LinuxMessage {
    Windows(Vec<HostWindow>),
    WatchInputDevices,
    InputDevices(Vec<HostInputDevice>),
    WatchDisplays,
    Displays(Vec<HostDisplay>),
    FullscreenWindow {
        window: HostWindowId,
        fullscreen: bool,
    },
}

impl LinuxMessage {
    pub fn direction(&self) -> Direction {
        match self {
            Self::Windows(_) | Self::InputDevices(_) | Self::Displays(_) => Direction::ToPlugin,
            Self::WatchInputDevices | Self::WatchDisplays | Self::FullscreenWindow { .. } => {
                Direction::ToHost
            }
        }
    }
}
