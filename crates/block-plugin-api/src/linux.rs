use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{ChildRect, HostAction, HostValue, Size};

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaLevel {
    pub level: f32,
    pub muted: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaLevels {
    pub output: Option<MediaLevel>,
    pub input: Option<MediaLevel>,
    pub brightness: Option<f32>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerCommand {
    PlayPause,
    Next,
    Previous,
    Stop,
}

impl PlayerCommand {
    pub const ALL: [Self; 4] = [Self::PlayPause, Self::Next, Self::Previous, Self::Stop];
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MediaRequest {
    StepVolume(f32),
    SetVolume(f32),
    ToggleMute,
    ToggleMicMute,
    StepBrightness(f32),
    Player(PlayerCommand),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostNotificationAction {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostNotification {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub received: u64,
    pub critical: bool,
    pub actions: Vec<HostNotificationAction>,
}

impl HostNotification {
    pub const DEFAULT_ACTION: &str = "default";

    pub fn has_default_action(&self) -> bool {
        self.actions
            .iter()
            .any(|action| action.key == Self::DEFAULT_ACTION)
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingDisplayChange {
    pub round: u64,
    pub timeout: Duration,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisplayAnswer {
    Keep(u64),
    Revert(u64),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostProblem {
    pub id: u64,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProblemAction {
    Dismiss(u64),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowAction {
    Focus(HostWindowId),
    Fullscreen {
        window: HostWindowId,
        fullscreen: bool,
    },
    Close(HostWindowId),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationAction {
    Invoke { id: u32, action: String },
    Dismiss(Vec<u32>),
}

pub enum HostWindows {}

impl HostValue for HostWindows {
    const KEY: &'static str = "windows";
    type Value = Vec<HostWindow>;
}

pub enum InputDevices {}

impl HostValue for InputDevices {
    const KEY: &'static str = "input_devices";
    type Value = Vec<HostInputDevice>;
}

pub enum Displays {}

impl HostValue for Displays {
    const KEY: &'static str = "displays";
    type Value = Vec<HostDisplay>;
}

pub enum Power {}

impl HostValue for Power {
    const KEY: &'static str = "power";
    type Value = PowerAvailability;
}

pub enum Media {}

impl HostValue for Media {
    const KEY: &'static str = "media";
    type Value = MediaLevels;
}

pub enum Notifications {}

impl HostValue for Notifications {
    const KEY: &'static str = "notifications";
    type Value = Vec<HostNotification>;
}

pub enum DisplayConfirmation {}

impl HostValue for DisplayConfirmation {
    const KEY: &'static str = "display_confirmation";
    type Value = Option<PendingDisplayChange>;
}

pub enum Problems {}

impl HostValue for Problems {
    const KEY: &'static str = "problems";
    type Value = Vec<HostProblem>;
}

impl HostAction for WindowAction {
    const KEY: &'static str = "window";
}

impl HostAction for PowerAction {
    const KEY: &'static str = "power";
}

impl HostAction for MediaRequest {
    const KEY: &'static str = "media";
}

impl HostAction for NotificationAction {
    const KEY: &'static str = "notification";
}

impl HostAction for DisplayAnswer {
    const KEY: &'static str = "display_answer";
}

impl HostAction for ProblemAction {
    const KEY: &'static str = "problem";
}
