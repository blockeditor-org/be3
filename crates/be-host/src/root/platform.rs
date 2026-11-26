//! What the host keeps of the platform that the root shows (plans/host_as_platform_adapter.md,
//! 4g7g), each answering the capability `be3:root` gives it: the session's screen lock with its
//! password check and power actions (`host.lock`), and the displays' keep changes
//! (`host.displays`). The lock is the login session's (`login_session::ScreenLock`, PAM), which
//! `--root` starts as be-host's own UI does: in a login session and under `--desktop`. Its
//! triggers lock it as under be-host's own UI (idle, logind, a suspend, Quick settings' Lock),
//! and Super+L through the root (`host.lock-session`), which takes every key.

use std::time::Duration;

use be_wasm_root_api::DisplayQuestion;
#[cfg(target_os = "linux")]
use be_wasm_root_api::{LockState, PowerAction};
use be_wasm_root_host::Root;

use crate::HostCore;

/// What the root was last shown of the platform.
#[derive(Default)]
pub(super) struct Platform {
    /// Whether the root was shown the session locked before its last frame, which therefore
    /// drew its lock screen: the login session holds back a suspend until it has been drawn
    /// (`ScreenLock::note_drawn`).
    #[cfg(target_os = "linux")]
    locked_shown: bool,
    /// When the host next needs a frame for the lock or the displays, from the last frame: the
    /// core's own repaint requests are not the root's (`RootShell::update`), so these are asked
    /// for here.
    due: Option<Duration>,
}

impl Platform {
    /// After the root's frame at `time` (its frame clock's): does what it asked of the lock and
    /// the displays, then shows it both as they stand, which its next frame hears.
    pub(super) fn serve(&mut self, core: &mut HostCore, root: &mut Root, time: Duration) {
        // The host's clock and the root's are the same frame clock from different origins.
        let host_now = be_host_base::host::now();
        let on_roots = |at: Duration| nanos((time + at).saturating_sub(host_now));
        self.due = None;
        self.serve_lock(core, root, on_roots);
        let revert = serve_displays(core, root, on_roots);
        self.need(revert.map(|at| at.saturating_sub(host_now)));
    }

    /// When the host next needs a frame for the lock or the displays, as the last frame said.
    pub(super) fn due(&self) -> Option<Duration> {
        self.due
    }

    fn need(&mut self, after: Option<Duration>) {
        if let Some(after) = after {
            self.due = Some(self.due.map_or(after, |due| due.min(after)));
        }
    }

    #[cfg(target_os = "linux")]
    fn serve_lock(
        &mut self,
        core: &mut HostCore,
        root: &mut Root,
        on_roots: impl Fn(Duration) -> u64,
    ) {
        let asked = root.take_lock_asked();
        let unlock = root.take_unlock();
        let powers = root.take_lock_powers();
        let Some(lock) = &mut core.screen_lock else {
            // No lock: `be3:root` shows it unavailable and drops a password, so none came.
            return;
        };
        lock.note_drawn(self.locked_shown);
        if asked {
            lock.lock(crate::login_session::Trigger::Root);
        }
        if let Some(password) = unlock {
            // Checked on a thread of its own and zeroed once checked; how it went reaches the
            // root as the lock's next change.
            lock.submit(password);
        }
        let locked = lock.locked();
        for action in powers_to_do(locked, &lock_powers(core), powers) {
            core.request_power(action, crate::login_session::Trigger::Menu);
        }
        let Some(lock) = &core.screen_lock else {
            return;
        };
        let shown = LockState {
            generation: 0,
            available: true,
            locked: lock.locked(),
            checking: lock.checking(),
            refused: lock.refused().map(str::to_owned),
            user: lock.user().to_owned(),
            power: lock_powers(core)
                .into_iter()
                .filter_map(root_power)
                .collect(),
            retry_at: lock.retry_at().map(on_roots),
        };
        self.locked_shown = shown.locked;
        // A frame as the wait after failed passwords ends, which shows the lock with none; and
        // frames until the lock screen counts as drawn, which a suspend waits for.
        let now = be_host_base::host::now();
        let retry = lock.retry_at().map(|at| at.saturating_sub(now));
        let drawing = (lock.locked() && !lock.shown()).then_some(Duration::ZERO);
        self.need(retry);
        self.need(drawing);
        // Shown every frame, which tells `be3:root` the password it took has been taken: it
        // drops one handed over until then.
        root.show_lock(shown);
    }

    /// No screen lock off Linux: `be3:root` shows it unavailable and drops what is asked of it.
    #[cfg(not(target_os = "linux"))]
    fn serve_lock(
        &mut self,
        _core: &mut HostCore,
        root: &mut Root,
        _on_roots: impl Fn(Duration) -> u64,
    ) {
        root.take_lock_asked();
        root.take_lock_powers();
        drop(root.take_unlock());
    }
}

/// What the lock screen offers besides unlocking, as be-host's own lock screen does
/// (`HostCore::lock_view`).
#[cfg(target_os = "linux")]
fn lock_powers(core: &HostCore) -> Vec<be_sdk_api::PowerAction> {
    let offered = core.power.availability();
    [
        be_sdk_api::PowerAction::Suspend,
        be_sdk_api::PowerAction::Restart,
        be_sdk_api::PowerAction::PowerOff,
    ]
    .into_iter()
    .filter(|action| offered.allows(*action))
    .collect()
}

/// What of the power actions the root chose on its lock screen (`asked`) the host does: only
/// while the session is locked, and only those its lock screen offers.
#[cfg(target_os = "linux")]
fn powers_to_do(
    locked: bool,
    offered: &[be_sdk_api::PowerAction],
    asked: Vec<PowerAction>,
) -> Vec<be_sdk_api::PowerAction> {
    asked
        .into_iter()
        .map(sdk_power)
        .filter(|action| locked && offered.contains(action))
        .collect()
}

#[cfg(target_os = "linux")]
fn root_power(action: be_sdk_api::PowerAction) -> Option<PowerAction> {
    match action {
        be_sdk_api::PowerAction::Suspend => Some(PowerAction::Suspend),
        be_sdk_api::PowerAction::Restart => Some(PowerAction::Restart),
        be_sdk_api::PowerAction::PowerOff => Some(PowerAction::PowerOff),
        be_sdk_api::PowerAction::LogOut => Some(PowerAction::LogOut),
        be_sdk_api::PowerAction::Lock => None,
    }
}

#[cfg(target_os = "linux")]
fn sdk_power(action: PowerAction) -> be_sdk_api::PowerAction {
    match action {
        PowerAction::Suspend => be_sdk_api::PowerAction::Suspend,
        PowerAction::Restart => be_sdk_api::PowerAction::Restart,
        PowerAction::PowerOff => be_sdk_api::PowerAction::PowerOff,
        PowerAction::LogOut => be_sdk_api::PowerAction::LogOut,
    }
}

/// Does how the root answered the displays' question, then shows it the question as it stands:
/// asked while display settings that just changed wait to be kept, with when the host reverts
/// them on the root's clock (`HostCore::run_display_prompt` reverts them then).
/// Answers when, on the host's clock, the host reverts the settings it asks about, if it asks.
fn serve_displays(
    core: &mut HostCore,
    root: &mut Root,
    on_roots: impl Fn(Duration) -> u64,
) -> Option<Duration> {
    for (round, keep) in root.take_display_answers() {
        core.answer_display(round, keep);
    }
    let now = be_host_base::host::now();
    let (asking, revert) = match crate::display::prompt(now) {
        crate::display::Prompt::Asking { round, left } => (
            Some(DisplayQuestion {
                round,
                revert_at: on_roots(now + left),
            }),
            Some(now + left),
        ),
        crate::display::Prompt::Settled | crate::display::Prompt::Expired => (None, None),
    };
    root.show_displays(asking);
    revert
}

fn nanos(at: Duration) -> u64 {
    u64::try_from(at.as_nanos()).unwrap_or(u64::MAX)
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
