use std::time::Duration;

use block_plugin_api::{PowerAction, PowerAvailability};

pub(crate) const GRACE: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogindCall {
    Suspend,
    Reboot,
    PowerOff,
}

impl LogindCall {
    pub(crate) fn method(self) -> &'static str {
        match self {
            Self::Suspend => "Suspend",
            Self::Reboot => "Reboot",
            Self::PowerOff => "PowerOff",
        }
    }

    fn doing(self) -> &'static str {
        match self {
            Self::Suspend => "suspend",
            Self::Reboot => "restart",
            Self::PowerOff => "power off",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LogindAbilities {
    pub(crate) suspend: bool,
    pub(crate) reboot: bool,
    pub(crate) power_off: bool,
}

pub(crate) trait SessionControl {
    fn call(&mut self, call: LogindCall);
    fn windows(&self) -> usize;
    fn close_windows(&mut self);
    fn exit(&mut self);
    fn report(&mut self, problem: String);
}

#[derive(Clone, Copy, Debug)]
struct Ending {
    action: PowerAction,
    deadline: Duration,
    called: Option<LogindCall>,
}

#[derive(Debug, Default)]
pub(crate) struct Power {
    abilities: LogindAbilities,
    ending: Option<Ending>,
}

impl Power {
    pub(crate) fn availability(&self) -> PowerAvailability {
        let idle = self.ending.is_none();
        PowerAvailability {
            lock: false,
            suspend: idle && self.abilities.suspend,
            restart: idle && self.abilities.reboot,
            power_off: idle && self.abilities.power_off,
            log_out: idle,
        }
    }

    pub(crate) fn set_abilities(&mut self, abilities: LogindAbilities) {
        self.abilities = abilities;
    }

    pub(crate) fn request(
        &mut self,
        action: PowerAction,
        now: Duration,
        control: &mut impl SessionControl,
    ) -> Option<Duration> {
        if !self.availability().allows(action) {
            return None;
        }
        if action == PowerAction::Suspend {
            control.call(LogindCall::Suspend);
            return None;
        }
        control.close_windows();
        self.ending = Some(Ending {
            action,
            deadline: now + GRACE,
            called: None,
        });
        self.frame(now, control)
    }

    pub(crate) fn frame(
        &mut self,
        now: Duration,
        control: &mut impl SessionControl,
    ) -> Option<Duration> {
        let ending = self.ending.as_mut()?;
        if ending.called.is_some() {
            return None;
        }
        if control.windows() > 0 && now < ending.deadline {
            return Some(ending.deadline - now);
        }
        let call = match ending.action {
            PowerAction::Restart => LogindCall::Reboot,
            PowerAction::PowerOff => LogindCall::PowerOff,
            PowerAction::LogOut | PowerAction::Suspend | PowerAction::Lock => {
                self.ending = None;
                control.exit();
                return None;
            }
        };
        ending.called = Some(call);
        control.call(call);
        None
    }

    pub(crate) fn called(
        &mut self,
        call: LogindCall,
        result: Result<(), String>,
        control: &mut impl SessionControl,
    ) {
        let ending = self
            .ending
            .is_some_and(|ending| ending.called == Some(call));
        if ending {
            self.ending = None;
        }
        match result {
            Ok(()) if ending => control.exit(),
            Ok(()) => {}
            Err(problem) => control.report(format!("Could not {}: {problem}", call.doing())),
        }
    }
}

#[cfg(test)]
mod tests;
