use serde::{Deserialize, Serialize};

use crate::{ChildRect, HostAction, HostImage, HostValue, Size};

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

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioOutput {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MediaLevels {
    pub output: Option<MediaLevel>,
    pub input: Option<MediaLevel>,
    pub brightness: Option<f32>,
    pub outputs: Vec<AudioOutput>,
    pub default_output: Option<String>,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum MediaRequest {
    StepVolume(f32),
    SetVolume(f32),
    ToggleMute,
    SetMute(bool),
    ChooseOutput(String),
    ToggleMicMute,
    StepBrightness(f32),
    Player(PlayerCommand),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostNotificationAction {
    pub key: String,
    pub label: String,
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationUrgency {
    Low,
    #[default]
    Normal,
    Critical,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncomingNotification {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<HostNotificationAction>,
    pub urgency: NotificationUrgency,
    pub image: Option<HostImage>,
    pub transient: bool,
    pub resident: bool,
    pub expire_timeout: i32,
    pub received: u64,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationRequest {
    Notify(Box<IncomingNotification>),
    Close(u32),
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationInbox {
    pub requests: Vec<(u64, NotificationRequest)>,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationCloseReason {
    Expired = 1,
    Dismissed = 2,
    Closed = 3,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationSignal {
    Closed(u32, NotificationCloseReason),
    ActionInvoked(u32, String),
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotificationReport {
    pub received: Option<u64>,
    pub signals: Vec<NotificationSignal>,
    pub kept: Vec<u32>,
}

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostProgram {
    pub id: String,
    pub name: String,
    pub generic_name: String,
    pub comment: String,
    pub keywords: Vec<String>,
    pub icon: Option<HostImage>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgramAction {
    List { icon_size: u32 },
    Launch(String),
    Run(String),
}

impl ProgramAction {
    pub const MAX_ICON_SIZE: u32 = 256;
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
    type Value = NotificationInbox;
}

pub enum ScreenLocked {}

impl HostValue for ScreenLocked {
    const KEY: &'static str = "screen_locked";
    type Value = bool;
}

pub enum Programs {}

impl HostValue for Programs {
    const KEY: &'static str = "programs";
    type Value = Vec<HostProgram>;
}

impl HostAction for ProgramAction {
    const KEY: &'static str = "program";
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

impl HostAction for NotificationReport {
    const KEY: &'static str = "notification";
}
