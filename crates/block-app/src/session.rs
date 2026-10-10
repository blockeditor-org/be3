mod cover;
mod install;
mod lock;
mod log;
mod logind;
mod pam;
mod power;
mod sleep;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use block_plugin_api::{PowerAction, PowerAvailability};

use cover::CoverChoice;
pub(crate) use cover::{Cover, PluginLock};

use crate::host::{WakingSender, waking_channel};
pub(crate) use install::install;
use lock::{Authenticate, Lock, Verdict};
pub(crate) use lock::{LockState, Password, Trigger};
pub(crate) use log::start_log;
use logind::{Logind, LogindEvent};
use power::{LogindCall, Power, SessionControl};
use sleep::{Inhibit, SleepGuard};

const SHOWN_FRAMES: u32 = 2;
const SECOND: Duration = Duration::from_secs(1);

pub(crate) fn owns_a_seat(setup: &beui::Setup) -> bool {
    setup.get::<beui_adapter_drm::DisplayControl>().is_some()
}

pub(crate) struct ScreenLock {
    lock: Lock,
    pam: Pam,
    user: String,
    drawn: u32,
    choice: CoverChoice,
    cover: Cover,
}

impl ScreenLock {
    pub(crate) fn start() -> Self {
        let user = pam::user();
        if user.is_none() {
            eprintln!(
                "block-app: this user has no account entry, so the screen cannot be unlocked"
            );
        }
        let shown = user
            .as_ref()
            .map(|(_, shown)| shown.clone())
            .unwrap_or_default();
        Self {
            lock: Lock::default(),
            pam: Pam::new(user.map(|(name, _)| name)),
            user: shown,
            drawn: 0,
            choice: CoverChoice::default(),
            cover: Cover::Blank,
        }
    }

    pub(crate) fn lock(&mut self, trigger: Trigger) {
        if self.lock.lock(trigger) {
            crate::wayland::set_locked(true);
            crate::host::request_repaint();
        }
    }

    pub(crate) fn locked(&self) -> bool {
        self.lock.locked()
    }

    pub(crate) fn submit_from_desktop(&mut self, password: Password) {
        if self.cover.takes_desktop_passwords() {
            self.submit(password);
        }
    }

    pub(crate) fn cover(&self) -> Cover {
        self.cover
    }

    pub(crate) fn choose_cover(&mut self, plugin: PluginLock) {
        let (cover, wake) = self
            .choice
            .choose(self.locked(), crate::host::now(), plugin);
        if cover != self.cover {
            self.cover = cover;
            crate::host::request_repaint();
        }
        if let Some(wake) = wake {
            crate::host::request_repaint_after(wake);
        }
    }

    pub(crate) fn published(&self) -> block_plugin_api::LockState {
        let state = self.state();
        block_plugin_api::LockState {
            locked: state.locked,
            user: self.user.clone(),
            checking: state.checking,
            retry_in_seconds: state.retry_in_seconds(),
            error: state.error,
        }
    }

    pub(crate) fn submit(&mut self, password: Password) {
        if let Some(wait) = self
            .lock
            .submit(password, crate::host::now(), &mut self.pam)
        {
            crate::host::request_repaint_after(until_next_second(wait));
        }
    }

    pub(crate) fn frame(&mut self) {
        let now = crate::host::now();
        for (attempt, verdict) in self.pam.answers.try_iter().collect::<Vec<_>>() {
            self.lock.answered(attempt, verdict, now);
        }
        if let Some(wait) = self.lock.wait(now) {
            crate::host::request_repaint_after(until_next_second(wait));
        }
    }

    pub(crate) fn note_drawn(&mut self, shown: bool) {
        self.drawn = match self.locked() && shown {
            true => self.drawn.saturating_add(1),
            false => 0,
        };
    }

    fn shown(&self) -> bool {
        self.locked() && self.drawn >= SHOWN_FRAMES
    }

    pub(crate) fn state(&self) -> LockState {
        self.lock.state(crate::host::now())
    }

    pub(crate) fn user(&self) -> &str {
        &self.user
    }
}

fn until_next_second(wait: Duration) -> Duration {
    match wait.subsec_nanos() {
        0 => SECOND,
        nanos => Duration::from_nanos(u64::from(nanos)),
    }
}

struct Pam {
    user: Option<String>,
    service: &'static str,
    sender: WakingSender<(u64, Verdict)>,
    answers: Receiver<(u64, Verdict)>,
}

impl Pam {
    fn new(user: Option<String>) -> Self {
        let (sender, answers) = waking_channel();
        let service = pam::service();
        eprintln!("block-app: the lock screen checks passwords with PAM's {service} service");
        Self {
            user,
            service,
            sender,
            answers,
        }
    }
}

impl Authenticate for Pam {
    fn start(&mut self, attempt: u64, password: Password) {
        let Some(user) = self.user.clone() else {
            let _ = self.sender.send((
                attempt,
                Verdict::Failed("this user has no account entry".to_owned()),
            ));
            return;
        };
        let (sender, service) = (self.sender.clone(), self.service);
        let spawned = std::thread::Builder::new()
            .name("authenticate".to_owned())
            .spawn(move || {
                let checked = catch_unwind(AssertUnwindSafe(|| {
                    pam::authenticate(service, &user, password.as_str())
                }));
                let verdict = checked.unwrap_or_else(|_| {
                    Verdict::Failed("the check stopped unexpectedly".to_owned())
                });
                let _ = sender.send((attempt, verdict));
            });
        if let Err(error) = spawned {
            let _ = self.sender.send((
                attempt,
                Verdict::Failed(format!("the check did not start: {error}")),
            ));
        }
    }
}

pub(crate) struct DesktopSession {
    logind: Logind,
    power: Power,
    sleep: SleepGuard,
    requested: Option<PowerAction>,
    hinted: Option<bool>,
}

impl DesktopSession {
    pub(crate) fn start() -> Self {
        let logind = Logind::connect();
        let mut sleep = SleepGuard::default();
        sleep.hold(&mut Inhibitor(&logind));
        Self {
            logind,
            power: Power::default(),
            sleep,
            requested: None,
            hinted: None,
        }
    }

    pub(crate) fn request(&mut self, action: PowerAction) {
        self.requested = Some(action);
    }

    pub(crate) fn availability(&self) -> PowerAvailability {
        self.power.availability()
    }

    pub(crate) fn frame(&mut self, context: &beui::Context, lock: &mut ScreenLock) {
        let now = crate::host::now();
        let mut control = Live {
            logind: &self.logind,
            context,
        };
        for event in self.logind.events() {
            match event {
                LogindEvent::Abilities(abilities) => self.power.set_abilities(abilities),
                LogindEvent::Called(call, result) => self.power.called(call, result, &mut control),
                LogindEvent::PrepareForSleep(starting) => {
                    match starting {
                        true => eprintln!("block-app: logind is suspending the computer"),
                        false => eprintln!("block-app: the computer woke up"),
                    }
                    self.sleep
                        .prepare(starting, &mut lock.lock, &mut Inhibitor(&self.logind));
                    crate::host::request_repaint();
                }
                LogindEvent::Lock => lock.lock(Trigger::Logind),
            }
        }
        let wait = self
            .requested
            .take()
            .and_then(|action| self.power.request(action, now, &mut control));
        if let Some(wait) = self.power.frame(now, &mut control).or(wait) {
            crate::host::request_repaint_after(wait);
        }
        if self.sleep.waiting() {
            match lock.shown() {
                true => self.sleep.shown(&mut Inhibitor(&self.logind)),
                false => crate::host::request_repaint(),
            }
        }
        let locked = lock.locked();
        if self.hinted != Some(locked) {
            self.hinted = Some(locked);
            self.logind.set_locked_hint(locked);
        }
    }
}

struct Inhibitor<'a>(&'a Logind);

impl Inhibit for Inhibitor<'_> {
    fn take(&mut self) {
        self.0.inhibit_sleep();
    }

    fn release(&mut self) {
        self.0.release_sleep();
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
