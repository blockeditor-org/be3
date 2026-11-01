mod environment;
mod install;
mod lock;
mod log;
mod logind;
mod pam;
mod power;
mod sleep;

use std::time::Duration;

use block_plugin_api::{PowerAction, PowerAvailability};

use block_app_base::host::{Desk, Mailbox, Post};
use block_app_base::password::Password;
use block_app_desktop::desktop_host::{DesktopHost, Host};
pub(crate) use environment::export as export_environment;
pub(crate) use install::install;
use lock::{Authenticate, Lock, Verdict};
pub(crate) use lock::{LockState, Trigger};
pub(crate) use log::start_log;
use logind::Logind;
pub(crate) use logind::LogindEvent;
use power::{LogindCall, Power, SessionControl};
use sleep::{Inhibit, SleepGuard};

const SHOWN_FRAMES: u32 = 2;
const SECOND: Duration = Duration::from_secs(1);

pub(crate) fn owns_a_seat(setup: &beui::native::Setup) -> bool {
    setup.get::<beui_platform_drm::DisplayControl>().is_some()
}

pub(crate) struct ScreenLock {
    lock: Lock,
    desk: Desk<ScreenLock>,
    _answers: Mailbox,
    pam: Pam,
    user: String,
    drawn: u32,
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
        let desk = Desk::default();
        let (answers, post) = desk.mailbox(|lock: &mut ScreenLock, (attempt, verdict)| {
            lock.lock
                .answered(attempt, verdict, block_app_base::host::now());
        });
        Self {
            lock: Lock::default(),
            desk,
            _answers: answers,
            pam: Pam::new(user.map(|(name, _)| name), post),
            user: shown,
            drawn: 0,
        }
    }

    pub(crate) fn lock(&mut self, trigger: Trigger) {
        if self.lock.lock(trigger) {
            Host::set_locked(true);
            block_app_base::host::request_repaint();
        }
    }

    pub(crate) fn locked(&self) -> bool {
        self.lock.locked()
    }

    pub(crate) fn submit(&mut self, password: Password) {
        if let Some(wait) = self
            .lock
            .submit(password, block_app_base::host::now(), &mut self.pam)
        {
            block_app_base::host::request_repaint_after(until_next_second(wait));
        }
    }

    pub(crate) fn frame(&mut self) {
        let desk = self.desk;
        desk.deliver(self);
        if let Some(wait) = self.lock.wait(block_app_base::host::now()) {
            block_app_base::host::request_repaint_after(until_next_second(wait));
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
        self.lock.state(block_app_base::host::now())
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
    answers: Post<(u64, Verdict)>,
}

impl Pam {
    fn new(user: Option<String>, answers: Post<(u64, Verdict)>) -> Self {
        let service = pam::service();
        eprintln!("block-app: the lock screen checks passwords with PAM's {service} service");
        Self {
            user,
            service,
            answers,
        }
    }
}

impl Authenticate for Pam {
    fn start(&mut self, attempt: u64, password: Password) {
        let Some(user) = self.user.clone() else {
            self.answers.send((
                attempt,
                Verdict::Failed("this user has no account entry".to_owned()),
            ));
            return;
        };
        let (answers, service) = (self.answers.clone(), self.service);
        let spawned = std::thread::Builder::new()
            .name("authenticate".to_owned())
            .spawn(move || {
                let checked = block_app_base::panic_guard::contain(|| {
                    pam::authenticate(service, &user, password.as_str())
                });
                let verdict = checked.unwrap_or_else(|_| {
                    Verdict::Failed("the check stopped unexpectedly".to_owned())
                });
                answers.send((attempt, verdict));
            });
        if let Err(error) = spawned {
            self.answers.send((
                attempt,
                Verdict::Failed(format!("the check did not start: {error}")),
            ));
        }
    }
}

pub(crate) struct LoginSession {
    _events: Mailbox,
    logind: Logind,
    power: Power,
    sleep: SleepGuard,
    requested: Option<PowerAction>,
    hinted: Option<bool>,
}

impl LoginSession {
    pub(crate) fn start((events, post): (Mailbox, Post<LogindEvent>)) -> Self {
        let logind = Logind::connect(post);
        let mut sleep = SleepGuard::default();
        sleep.hold(&mut Inhibitor(&logind));
        Self {
            _events: events,
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

    pub(crate) fn heard(&mut self, event: LogindEvent, lock: &mut ScreenLock) {
        let mut control = Live {
            logind: &self.logind,
        };
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
                block_app_base::host::request_repaint();
            }
            LogindEvent::Lock => lock.lock(Trigger::Logind),
        }
    }

    pub(crate) fn frame(&mut self, lock: &mut ScreenLock) {
        let now = block_app_base::host::now();
        let mut control = Live {
            logind: &self.logind,
        };
        let wait = self
            .requested
            .take()
            .and_then(|action| self.power.request(action, now, &mut control));
        if let Some(wait) = self.power.frame(now, &mut control).or(wait) {
            block_app_base::host::request_repaint_after(wait);
        }
        if self.sleep.waiting() {
            match lock.shown() {
                true => self.sleep.shown(&mut Inhibitor(&self.logind)),
                false => block_app_base::host::request_repaint(),
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
}

impl SessionControl for Live<'_> {
    fn call(&mut self, call: LogindCall) {
        self.logind.call(call);
    }

    fn windows(&self) -> usize {
        Host::listed(&[]).len()
    }

    fn close_windows(&mut self) {
        for window in Host::listed(&[]) {
            Host::close(window.id);
        }
        block_app_base::host::wake();
    }

    fn exit(&mut self) {
        block_app_base::host::close_window();
    }

    fn report(&mut self, problem: String) {
        block_app_base::notices::report(problem);
    }
}
