//! The lock screen over everything while the session is locked.

use super::*;
use beui::datetime::{DateTime, HourCycle};

/// The lock screen, as be-host's own shows it: the time and date at the host's UTC offset
/// (`utc_offset`, seconds east of UTC), whose session it is, a password field it focuses, why
/// the last password was refused, and what the host offers besides unlocking. What is typed is
/// handed to the host to check, and the field cleared as it is.
#[component]
pub(super) fn LockCard(
    state: Signal<LockState>,
    shown_at: Signal<Duration>,
    utc_offset: i32,
) -> NodeId {
    let password = create_signal(String::new());
    let user = create_memo(move || state.with(|state| state.user.clone()));
    let checking = create_memo(move || state.with(|state| state.checking));
    let refused = create_memo(move || state.with(|state| state.refused.clone()));
    let powers = create_memo(move || state.with(|state| state.power.clone()));
    let wait = retry_wait(state, shown_at);
    let slots = create_memo(move || (0..powers.with(Vec::len)).collect::<Vec<usize>>());
    let focused = create_memo(move || !checking.get());
    let clock = clock(utc_offset);
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
    let submit = move |typed: String| {
        if typed.is_empty() || checking.get_untracked() {
            return;
        }
        password.set(String::new());
        unlock(typed);
    };
    view! {
        <Frame width=Length::FILL height=Length::FILL align=Align2::CENTER padding=Space::Lg>
            <List spacing=Space::Xl3 align=Align::Center>
                <List spacing=Space::Md align=Align::Center>
                    <Text style=TextStyle::DISPLAY>{time}</Text>
                    <Text color=TextColor::Muted>{date}</Text>
                </List>
                <List spacing=Space::Md align=Align::Center>
                    <Text style=TextStyle::TITLE>{user}</Text>
                    <Frame width={Size::Custom(280.0)}>
                        <TextInput
                            value={password}
                            placeholder="Password"
                            label="Password"
                            kind={InputKind::Password}
                            disabled={checking}
                            focused={focused}
                            on_submit={submit}
                        />
                    </Frame>
                    <Show value={move || checking.get() && refused.with(Option::is_none)}>
                        <Text style=TextStyle::CAPTION>"Checking…"</Text>
                    </Show>
                    <Show value={refused}>|reason: ReadSignal<String>|
                        <Text style=TextStyle::CAPTION color=TextColor::Danger>{reason}</Text>
                    </Show>
                    <Show value={wait}>|left: ReadSignal<String>|
                        <Text style=TextStyle::CAPTION>{left}</Text>
                    </Show>
                </List>
                <List direction=Direction::Horizontal spacing=Space::Xs>
                    <ForEach value={slots}>|index: usize| {
                        let action = create_memo(move || {
                            powers.with(|powers| powers.get(index).copied())
                        });
                        let label = create_memo(move || action.get().map(power_label).unwrap_or_default());
                        let glyph = create_memo(move || action.get().map(power_glyph));
                        view! {
                            <Button
                                icon_only=true
                                glyph
                                label
                                on_click={move || {
                                    if let Some(action) = action.get_untracked() {
                                        lock_power(action);
                                    }
                                }}
                            />
                        }
                    }</ForEach>
                </List>
            </List>
        </Frame>
    }
}

/// The wall clock at `utc_offset` seconds east of UTC, to the minute, ticking over as each
/// minute starts.
fn clock(utc_offset: i32) -> ReadSignal<DateTime> {
    const SECONDS_PER_MINUTE: u64 = 60;
    let unix = beui::reactive::wall_clock;
    let shown = move || {
        let seconds = i64::try_from(unix().as_secs()).unwrap_or(0) + i64::from(utc_offset);
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

/// "Try again in N s" while the host makes the person wait after failed passwords (`retry-at`,
/// on the host's frame clock), counted down a second at a time by a timer on beui's clock, which
/// the root pins to the host's: `shown_at` is the host's frame time when the state was shown.
/// None once the wait is over.
pub(super) fn retry_wait(
    state: Signal<LockState>,
    shown_at: Signal<Duration>,
) -> Memo<Option<String>> {
    let retry_at = create_memo(move || state.with(|state| state.retry_at));
    let left = create_signal(None::<u64>);
    // When the wait was last shown, on beui's clock and on the host's.
    let from = StoredCell::new((now(), Duration::ZERO));
    let tick = create_timer(move || {
        let at = retry_at.get_untracked()?;
        let (since, host) = from.get();
        let host_now = host + now().saturating_duration_since(since);
        let host_now = u64::try_from(host_now.as_nanos()).unwrap_or(u64::MAX);
        let seconds = at.saturating_sub(host_now).div_ceil(1_000_000_000);
        left.set((seconds > 0).then_some(seconds));
        (seconds > 0).then_some(Duration::from_secs(1))
    });
    create_effect(move || {
        retry_at.get();
        let host = shown_at.get();
        untrack(|| {
            from.set((now(), host));
            left.set(None);
            tick.restart(Duration::ZERO);
        });
    });
    create_memo(move || {
        left.get()
            .map(|seconds| format!("Try again in {seconds} s"))
    })
}

/// Hands `typed` to the host to check (`host.unlock`), keeping no copy: whether it unlocked the
/// session reaches the root as the lock's next change, and why it was refused with it.
pub(super) fn unlock(typed: String) {
    let password = Password::from(typed);
    be_wasm_ops::guest::spawn(async move {
        // A host that cannot hear it has stopped, which the root hears from its frames.
        let _ = host::unlock(password).await;
    });
}

/// Tells the host the person chose `action` on the lock screen.
pub(super) fn lock_power(action: PowerAction) {
    be_wasm_ops::guest::spawn(async move {
        // A host that cannot hear it has stopped, which the root hears from its frames.
        let _ = host::lock_power(action).await;
    });
}

pub(super) fn power_label(action: PowerAction) -> String {
    match action {
        PowerAction::Suspend => "Suspend",
        PowerAction::Restart => "Restart",
        PowerAction::PowerOff => "Power off",
        PowerAction::LogOut => "Log out",
    }
    .to_owned()
}

pub(super) fn power_glyph(action: PowerAction) -> Glyph {
    match action {
        PowerAction::Suspend => ICON_MOON,
        PowerAction::Restart => ICON_ROTATE_CCW,
        PowerAction::PowerOff => ICON_POWER,
        PowerAction::LogOut => ICON_LOG_OUT,
    }
    .to_owned()
}
