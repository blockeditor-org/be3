use crate::{
    DecodeError, HostImage, HostTrayItem, NotificationCloseReason, NotificationUrgency,
    PlayerCommand, PowerAction, PowerAvailability, ProgramAction, TrayMenuItem, TrayStatus,
};

impl HostImage {
    pub fn is_whole(&self) -> bool {
        self.rgba.len() == self.width as usize * self.height as usize * 4
    }
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

impl PlayerCommand {
    pub const ALL: [Self; 4] = [Self::PlayPause, Self::Next, Self::Previous, Self::Stop];
}

impl Default for NotificationUrgency {
    fn default() -> Self {
        Self::Normal
    }
}

impl NotificationCloseReason {
    pub fn code(self) -> u32 {
        match self {
            Self::Expired => 1,
            Self::Dismissed => 2,
            Self::Closed => 3,
        }
    }
}

impl ProgramAction {
    pub const MAX_ICON_SIZE: u32 = 256;
}

impl Default for TrayStatus {
    fn default() -> Self {
        Self::Active
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayMenuEntry {
    pub item: TrayMenuItem,
    pub children: Vec<TrayMenuEntry>,
}

impl TrayMenuEntry {
    pub fn flatten(entries: &[TrayMenuEntry]) -> Vec<TrayMenuItem> {
        crate::nesting::flatten(
            entries,
            |entry| &entry.children,
            |entry, depth| TrayMenuItem {
                depth,
                ..entry.item.clone()
            },
        )
    }
}

impl HostTrayItem {
    pub fn entries(&self) -> Result<Vec<TrayMenuEntry>, DecodeError> {
        crate::nesting::nest(
            self.menu.clone(),
            |item| item.depth,
            |item, children| {
                Ok(TrayMenuEntry {
                    item: TrayMenuItem { depth: 0, ..item },
                    children,
                })
            },
        )
    }
}
