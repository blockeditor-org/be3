use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::HourCycle;
use block_editor_beui::beui::icons::{ICON_BEDTIME, ICON_POWER_SETTINGS_NEW, ICON_RESTART_ALT};
use block_editor_beui::beui::reactive::{
    Frame, clone, component, create_effect, create_memo, view,
};
use block_editor_beui::beui::styled::{LockAction, LockCards, use_theme};
use block_editor_beui::{Editor, Idle, LockState, Power, PowerAction, ScreenLock, UnlockAttempt};

use super::bar::wall_clock;

pub(crate) const LOCK_POWER: [PowerAction; 3] = [
    PowerAction::Suspend,
    PowerAction::Restart,
    PowerAction::PowerOff,
];

pub(crate) fn bind_lock(editor: &Editor) {
    let idle = editor.host_value::<Idle>();
    let due = create_memo(move || idle.get().lock_due);
    let locking = editor.clone();
    create_effect(move || {
        if due.get() {
            locking.act(PowerAction::Lock);
        }
    });
}

pub(crate) fn message(state: &LockState) -> Option<String> {
    match state.retry_in_seconds {
        Some(1) => Some("Too many attempts. Try again in 1 second.".to_owned()),
        Some(seconds) => Some(format!(
            "Too many attempts. Try again in {seconds} seconds."
        )),
        None => state.error.clone(),
    }
}

fn action(power: PowerAction) -> LockAction {
    let (label, glyph) = match power {
        PowerAction::Suspend => ("Suspend", ICON_BEDTIME),
        PowerAction::Restart => ("Restart", ICON_RESTART_ALT),
        _ => ("Power off", ICON_POWER_SETTINGS_NEW),
    };
    LockAction {
        label: label.to_owned(),
        glyph: glyph.to_owned(),
    }
}

#[component]
pub(crate) fn DesktopLock(editor: Editor) -> NodeId {
    let theme = use_theme();
    let lock = editor.host_value::<ScreenLock>();
    let power = editor.host_value::<Power>();
    let wall = wall_clock();
    let time = create_memo(clone!(wall -> move || wall.get().time.format(HourCycle::H24)));
    let date = create_memo(move || {
        let date = wall.get().date;
        format!(
            "{}, {} {}",
            date.weekday().name(),
            date.day,
            date.month_name()
        )
    });
    let user = create_memo(clone!(lock -> move || lock.get().user));
    let error = create_memo(clone!(lock -> move || lock.with(message)));
    let busy = create_memo(clone!(lock -> move || {
        lock.with(|state| state.checking || state.retry_in_seconds.is_some())
    }));
    let active = create_memo(clone!(lock -> move || lock.get().locked));
    let offered = create_memo(move || {
        let power = power.get();
        LOCK_POWER
            .into_iter()
            .filter(|action| power.allows(*action))
            .collect::<Vec<_>>()
    });
    let actions = create_memo(clone!(offered -> move || {
        offered.with(|offered| offered.iter().copied().map(action).collect::<Vec<_>>())
    }));
    let checking = editor.clone();
    let asking = editor;
    view! {
        <Frame color={theme.background.clone()}>
            <LockCards
                time
                date
                user
                error
                busy
                actions
                active
                id="desktop.lock"
                on_submit={move |password: String| checking.act(UnlockAttempt { password })}
                on_action={move |index: usize| {
                    if let Some(power) = offered.with_untracked(|offered| offered.get(index).copied()) {
                        asking.act(power);
                    }
                }}
            />
        </Frame>
    }
}
