mod install;
mod log;
mod logind;
mod power;

use block_plugin_api::{PowerAction, PowerAvailability};

pub(crate) use install::install;
pub(crate) use log::start_log;
use logind::{Logind, LogindEvent};
use power::{LogindCall, Power, SessionControl};

pub(crate) fn owns_a_seat(setup: &beui::Setup) -> bool {
    setup.get::<beui_adapter_drm::DisplayControl>().is_some()
}

pub(crate) struct DesktopSession {
    logind: Logind,
    power: Power,
    published: Option<PowerAvailability>,
}

impl DesktopSession {
    pub(crate) fn start() -> Self {
        Self {
            logind: Logind::connect(),
            power: Power::default(),
            published: None,
        }
    }

    pub(crate) fn frame(&mut self, context: &beui::Context, request: Option<PowerAction>) {
        let now = crate::host::now();
        let mut control = Live {
            logind: &self.logind,
            context,
        };
        for event in self.logind.events() {
            match event {
                LogindEvent::Abilities(abilities) => self.power.set_abilities(abilities),
                LogindEvent::Called(call, result) => self.power.called(call, result, &mut control),
                LogindEvent::PrepareForSleep(starting) => prepare_for_sleep(starting),
            }
        }
        let wait = request.and_then(|action| self.power.request(action, now, &mut control));
        if let Some(wait) = self.power.frame(now, &mut control).or(wait) {
            crate::host::request_repaint_after(wait);
        }
        let availability = self.power.availability();
        if self.published != Some(availability) {
            self.published = Some(availability);
            crate::plugin_host::set_power(availability);
        }
    }
}

fn prepare_for_sleep(starting: bool) {
    match starting {
        true => eprintln!("block-app: logind is suspending the computer"),
        false => eprintln!("block-app: the computer woke up"),
    }
}

struct Live<'a> {
    logind: &'a Logind,
    context: &'a beui::Context,
}

impl SessionControl for Live<'_> {
    fn call(&mut self, call: LogindCall) {
        self.logind.call(call);
    }

    fn windows(&self) -> usize {
        crate::wayland::listed().len()
    }

    fn close_windows(&mut self) {
        for window in crate::wayland::listed() {
            crate::wayland::close(window.id);
        }
        crate::host::wake();
    }

    fn exit(&mut self) {
        self.context.close_window();
    }

    fn report(&mut self, problem: String) {
        crate::notices::report(problem);
    }
}
