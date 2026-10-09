use std::time::{Duration, SystemTime, UNIX_EPOCH};

use beui::datetime::{DateTime, HourCycle};
use beui::icons::{ICON_BEDTIME, ICON_LOCK, ICON_POWER_SETTINGS_NEW, ICON_RESTART_ALT};
use beui::reactive::{
    Action, Chord, ReadSignal, clone, component, create_memo, create_signal, create_timer, now,
    view,
};
use beui::styled::{LockAction, LockScreen};
use beui::{Key, NodeId};
use block_plugin_api::PowerAction;

use super::{AppViewStore, UiCommand, send};
use crate::password::Password;

const SECONDS_PER_MINUTE: u64 = 60;

fn action(power: PowerAction) -> LockAction {
    let (label, glyph) = match power {
        PowerAction::Lock => ("Lock", ICON_LOCK),
        PowerAction::Suspend => ("Suspend", ICON_BEDTIME),
        PowerAction::Restart => ("Restart", ICON_RESTART_ALT),
        PowerAction::PowerOff | PowerAction::LogOut => ("Power off", ICON_POWER_SETTINGS_NEW),
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
        let seconds = i64::try_from(unix().as_secs()).unwrap_or(0)
            + i64::from(crate::platform::utc_offset());
        DateTime::from_unix(seconds)
    };
    let until_next_minute = move || {
        let into = unix().as_secs() % SECONDS_PER_MINUTE;
        Duration::from_secs(SECONDS_PER_MINUTE - into)
    };
    let (clock, set_clock) = create_signal(shown());
    let ticking = create_timer(move || {
        set_clock.set(shown());
        Some(until_next_minute())
    });
    ticking.start(until_next_minute());
    clock
}

#[component]
pub(crate) fn SessionLock(view: AppViewStore) -> NodeId {
    let lock = view.lock.clone();
    let screens = view.screens.clone();
    let available = create_memo(clone!(lock -> move || lock.get().available && !lock.get().locked));
    Action::new("session.lock", "Lock the screen", || {
        send(UiCommand::LockScreen);
    })
    .glyph(ICON_LOCK)
    .shortcut(Chord::logo(Key::L))
    .intercepts()
    .enabled(available)
    .register();
    let clock = wall_clock();
    let time = create_memo(clone!(clock -> move || clock.get().time.format(HourCycle::H24)));
    let date = create_memo(move || {
        let date = clock.get().date;
        format!(
            "{}, {} {}",
            date.weekday().name(),
            date.day,
            date.month_name()
        )
    });
    let open = create_memo(clone!(lock -> move || lock.get().locked));
    let user = create_memo(clone!(lock -> move || lock.get().user));
    let error = create_memo(clone!(lock -> move || lock.get().error));
    let busy = create_memo(clone!(lock -> move || lock.get().busy));
    let powers = create_memo(clone!(lock -> move || lock.get().power));
    let actions = create_memo(clone!(powers -> move || {
        powers.with(|powers| powers.iter().copied().map(action).collect::<Vec<_>>())
    }));
    view! {
        <LockScreen
            open
            time
            date
            user
            error
            busy
            screens
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
