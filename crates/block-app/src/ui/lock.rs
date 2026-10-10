use std::time::{Duration, SystemTime, UNIX_EPOCH};

use beui::datetime::{DateTime, HourCycle};
use beui::reactive::{
    Frame, Layers, Overlay, OverlayAnchor, Placement, ReadSignal, Show, clone, component,
    create_memo, create_signal, create_timer, now, use_screens, view,
};
use beui::styled::{LockCards, use_theme};
use beui::{NodeId, screen_bounds};

use super::{AppViewStore, LockCover, UiCommand, send};
use crate::compositor::LockSurface;
use crate::password::Password;

const SECONDS_PER_MINUTE: u64 = 60;

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
    let (clock, set_clock) = create_signal(shown());
    let ticking = create_timer(move || {
        set_clock.set(shown());
        Some(until_next_minute())
    });
    ticking.start(until_next_minute());
    clock
}

#[component]
pub(crate) fn ScreenCover(view: AppViewStore) -> NodeId {
    let theme = use_theme();
    let lock = view.lock.clone();
    let open = create_memo(clone!(lock -> move || lock.get().locked));
    let cover = create_memo(clone!(lock -> move || lock.get().cover));
    let desktop = create_memo(clone!(cover -> move || cover.get() == LockCover::Desktop));
    let built_in = create_memo(clone!(cover -> move || cover.get() == LockCover::BuiltIn));
    let screens = use_screens();
    let bounds =
        create_memo(clone!(screens -> move || screens.with(|screens| screen_bounds(screens))));
    let anchor = create_memo(clone!(bounds -> move || OverlayAnchor::Point(bounds.get().min)));
    let width = create_memo(clone!(bounds -> move || Some(bounds.get().width())));
    let height = create_memo(clone!(bounds -> move || Some(bounds.get().height())));
    view! {
        <Overlay
            anchor={anchor}
            open={open.clone()}
            placement=Placement::At
            scrim={theme.background.clone()}
            locks=true
        >
            <Frame width={width} height={height} color={theme.background.clone()}>
                <Layers>
                    <Show condition={open}>
                        {move || clone!(desktop built_in view -> view! {
                            <Layers>
                                <LockSurface shown={desktop} />
                                <Show condition={built_in}>
                                    {move || clone!(view -> view! {
                                        <BuiltInLock view />
                                    })}
                                </Show>
                            </Layers>
                        })}
                    </Show>
                </Layers>
            </Frame>
        </Overlay>
    }
}

#[component]
fn BuiltInLock(view: AppViewStore) -> NodeId {
    let theme = use_theme();
    let lock = view.lock.clone();
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
    let user = create_memo(clone!(lock -> move || lock.get().user));
    let error = create_memo(clone!(lock -> move || lock.get().error));
    let busy = create_memo(clone!(lock -> move || lock.get().busy));
    view! {
        <Frame color={theme.background.clone()}>
            <LockCards
                time
                date
                user
                error
                busy
                id="session.lock"
                on_submit={|typed: String| send(UiCommand::Unlock(Password::new(typed)))}
                on_action={|_index: usize| {}}
            />
        </Frame>
    }
}
