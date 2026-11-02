#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PowerAction {
    Lock,
    Suspend,
    Restart,
    PowerOff,
    LogOut,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
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
