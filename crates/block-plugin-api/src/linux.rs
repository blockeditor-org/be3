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
    pub responding: bool,
    pub focused: bool,
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

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerAction {
    Lock,
    Suspend,
    Restart,
    PowerOff,
    LogOut,
}

impl PowerAction {
    pub const ALL: [Self; 5] = [
        Self::Lock,
        Self::Suspend,
        Self::Restart,
        Self::PowerOff,
        Self::LogOut,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerAvailability {
    pub lock: bool,
    pub suspend: bool,
    pub restart: bool,
    pub power_off: bool,
    pub log_out: bool,
}

impl PowerAvailability {
    pub fn allows(&self, action: PowerAction) -> bool {
        match action {
            PowerAction::Lock => self.lock,
            PowerAction::Suspend => self.suspend,
            PowerAction::Restart => self.restart,
            PowerAction::PowerOff => self.power_off,
            PowerAction::LogOut => self.log_out,
        }
    }
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
    FocusWindow(HostWindowId),
    WatchPower,
    Power(PowerAvailability),
    RequestPower(PowerAction),
}

impl LinuxMessage {
    pub fn direction(&self) -> Direction {
        match self {
            Self::Windows(_) | Self::InputDevices(_) | Self::Displays(_) | Self::Power(_) => {
                Direction::ToPlugin
            }
            Self::WatchInputDevices
            | Self::WatchDisplays
            | Self::FullscreenWindow { .. }
            | Self::FocusWindow(_)
            | Self::WatchPower
            | Self::RequestPower(_) => Direction::ToHost,
        }
    }
}
