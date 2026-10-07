use serde::{Deserialize, Serialize};

use crate::{Direction, Size};

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HostWindowId(pub u64);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostWindow {
    pub id: HostWindowId,
    pub title: String,
    pub app_id: String,
    pub parent: Option<HostWindowId>,
    pub size: Size,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LinuxMessage {
    Windows(Vec<HostWindow>),
    WatchInputDevices,
    InputDevices(Vec<HostInputDevice>),
}

impl LinuxMessage {
    pub fn direction(&self) -> Direction {
        match self {
            Self::Windows(_) | Self::InputDevices(_) => Direction::ToPlugin,
            Self::WatchInputDevices => Direction::ToHost,
        }
    }
}
