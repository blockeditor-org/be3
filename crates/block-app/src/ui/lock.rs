use beui::KeyAt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use beui::datetime::{DateTime, HourCycle};
use beui::icons::{ICON_LOCK, ICON_LOG_OUT, ICON_MOON, ICON_POWER, ICON_ROTATE_CCW};
use beui::reactive::*;
use beui::styled::*;
use beui::{Key, NodeId};
use block_plugin_api::PowerAction;

use super::{AppViewStore, UiCommand, send};
use crate::password::Password;

const SECONDS_PER_MINUTE: u64 = 60;

fn action(power: PowerAction) -> LockAction {
    let (label, glyph) = match power {
        PowerAction::Lock => ("Lock", ICON_LOCK),
        PowerAction::Suspend => ("Suspend", ICON_MOON),
        PowerAction::Restart => ("Restart", ICON_ROTATE_CCW),
        PowerAction::PowerOff => ("Power off", ICON_POWER),
        PowerAction::LogOut => ("Log out", ICON_LOG_OUT),
    };
    LockAction {
        label: label.to_owned(),
        glyph: glyph.to_owned(),
    }
}

fn wall_clock() -> ReadSignal<DateTime> {
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let anchor = now();
    let unix = move || started + now().saturating_duration_since(anchor);
    let shown = move || {
        let seconds =
            i64::try_from(unix().as_secs()).unwrap_or(0) + i64::from(crate::platform::utc_offset());
        DateTime::from_unix(seconds)
    };
    let until_next_minute = move || {
        let into = unix().as_secs() % SECONDS_PER_MINUTE;
        Duration::from_secs(SECONDS_PER_MINUTE - into)
    };
    let clock = create_signal(shown());
    let ticking = create_timer(move || {
        clock.set(shown());
        Some(until_next_minute())
    });
    ticking.start(until_next_minute());
    clock.read_only()
}

#[component]
pub(crate) fn SessionLock(
    view: AppViewStore,
    #[prop(default = true)] shortcut: bool,
) -> NodeId {
    let lock = view.lock;
    let available = create_memo(move || lock.get().available && !lock.get().locked);
    if shortcut {
        Action::new("session.lock", "Lock the screen", || {
            send(UiCommand::LockScreen);
        })
        .glyph(ICON_LOCK)
        .shortcut(Chord::logo(Key::L))
        .at(KeyAt::Host)
        .capture()
        .enabled(available)
        .register();
    }
    let clock = wall_clock();
    let time = create_memo(move || clock.get().time.format(HourCycle::H24));
    let date = create_memo(move || {
        let date = clock.get().date;
        format!(
            "{}, {} {}",
            date.weekday().name(),
            date.day,
            date.month_name()
        )
    });
    let open = create_memo(move || lock.get().locked);
    let user = create_memo(move || lock.get().user);
    let error = create_memo(move || lock.get().error);
    let busy = create_memo(move || lock.get().busy);
    let powers = create_memo(move || lock.get().power);
    let actions = create_memo(move || {
        powers.with(|powers| powers.iter().copied().map(action).collect::<Vec<_>>())
    });
    view! {
        <LockScreen
            open
            time
            date
            user
            error
            busy
            actions
            id="session.lock"
            on_submit={|typed: String| send(UiCommand::Unlock(Password::new(typed)))}
            on_action={move |index: usize| {
                if let Some(power) = powers.with_untracked(|powers| powers.get(index).copied()) {
                    send(UiCommand::LockPower(power));
                }
            }}
        />
    }
}
