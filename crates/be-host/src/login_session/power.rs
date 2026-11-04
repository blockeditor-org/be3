use be_sdk_api::{EndingAnswer, PowerAction, PowerAvailability, SessionEnd};

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
    end: SessionEnd,
    asked: bool,
    forced: bool,
    called: Option<LogindCall>,
}

#[derive(Debug, Default)]
pub(crate) struct Power {
    abilities: LogindAbilities,
    logged_in: bool,
    ending: Option<Ending>,
    exiting: bool,
}

impl Power {
    pub(crate) fn availability(&self) -> PowerAvailability {
        let idle = self.ending.is_none() && !self.exiting;
        PowerAvailability {
            lock: false,
            suspend: idle && self.abilities.suspend,
            restart: idle && self.abilities.reboot,
            power_off: idle && self.abilities.power_off,
            log_out: idle && self.logged_in,
            session: self.logged_in,
            ending: self.ending.map(|ending| ending.end),
        }
    }

    pub(crate) fn set_abilities(&mut self, abilities: LogindAbilities) {
        self.abilities = abilities;
    }

    pub(crate) fn log_in(&mut self) {
        self.logged_in = true;
    }

    pub(crate) fn request(&mut self, action: PowerAction, control: &mut impl SessionControl) {
        if !self.availability().allows(action) {
            return;
        }
        let end = match action {
            PowerAction::Lock => return,
            PowerAction::Suspend => {
                control.call(LogindCall::Suspend);
                return;
            }
            PowerAction::LogOut => SessionEnd::LogOut,
            PowerAction::Restart => SessionEnd::Restart,
            PowerAction::PowerOff => SessionEnd::PowerOff,
        };
        self.begin(end);
    }

    pub(crate) fn close(&mut self, control: &mut impl SessionControl) -> bool {
        if self.exiting {
            return true;
        }
        if self.ending.is_some() {
            return false;
        }
        if control.windows() == 0 {
            return true;
        }
        self.begin(match self.logged_in {
            true => SessionEnd::LogOut,
            false => SessionEnd::Close,
        });
        false
    }

    pub(crate) fn stay(&mut self) {
        self.exiting = false;
    }

    fn begin(&mut self, end: SessionEnd) {
        self.ending = Some(Ending {
            end,
            asked: false,
            forced: false,
            called: None,
        });
    }

    pub(crate) fn answer(&mut self, answer: EndingAnswer) {
        let Some(ending) = self.ending.as_mut() else {
            return;
        };
        if ending.called.is_some() {
            return;
        }
        match answer {
            EndingAnswer::Cancel => self.ending = None,
            EndingAnswer::EndNow => ending.forced = true,
        }
    }

    pub(crate) fn frame(&mut self, control: &mut impl SessionControl) {
        let Some(ending) = self.ending.as_mut() else {
            return;
        };
        if ending.called.is_some() {
            return;
        }
        if !ending.asked {
            ending.asked = true;
            control.close_windows();
        }
        if control.windows() > 0 && !ending.forced {
            return;
        }
        let call = match ending.end {
            SessionEnd::Restart => LogindCall::Reboot,
            SessionEnd::PowerOff => LogindCall::PowerOff,
            SessionEnd::LogOut | SessionEnd::Close => {
                self.end(control);
                return;
            }
        };
        ending.called = Some(call);
        control.call(call);
    }

    fn end(&mut self, control: &mut impl SessionControl) {
        self.ending = None;
        self.exiting = true;
        control.exit();
    }

    pub(crate) fn called(
        &mut self,
        call: LogindCall,
        result: Result<(), String>,
        control: &mut impl SessionControl,
    ) {
        let ours = self
            .ending
            .is_some_and(|ending| ending.called == Some(call));
        match result {
            Ok(()) if ours => self.end(control),
            Ok(()) => {}
            Err(problem) => {
                if ours {
                    self.ending = None;
                }
                control.report(format!("Could not {}: {problem}", call.doing()));
            }
        }
    }
}

#[cfg(test)]
mod tests;
