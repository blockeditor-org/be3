use beui::KeyAt;
use beui::LayerLevel;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use beui::datetime::{DateTime, HourCycle};
use beui::icons::{Glyph, ICON_LOCK, ICON_LOG_OUT, ICON_MOON, ICON_POWER, ICON_ROTATE_CCW};
use beui::reactive::*;
use beui::styled::*;
use beui::{Key, NodeId};
use block_plugin_api::PowerAction;

use super::{AppViewStore, UiCommand, send};
use crate::password::Password;

const SECONDS_PER_MINUTE: u64 = 60;
const FIELD_WIDTH: Size = Size::Custom(280.0);
const SPACING: Space = Space::Md;
const GROUP_SPACING: Space = Space::Xl3;
const ACTION_SPACING: Space = Space::Xs;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub(crate) struct LockAction {
    pub(crate) label: String,
    pub(crate) glyph: Glyph,
}

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
pub(crate) fn SessionLock(view: AppViewStore) -> NodeId {
    let lock = view.lock;
    let available = create_memo(move || lock.get().available && !lock.get().locked);
    Action::new("session.lock", "Lock the screen", || {
        send(UiCommand::LockScreen);
    })
    .glyph(ICON_LOCK)
    .shortcut(Chord::logo(Key::L))
    .at(KeyAt::Host)
    .capture()
    .enabled(available)
    .register();
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

#[component]
pub(crate) fn LockScreen(
    open: Prop<bool>,
    time: Prop<String>,
    date: Prop<String>,
    user: Prop<String>,
    #[prop(default = None)] error: Prop<Option<String>>,
    #[prop(default = false)] busy: Prop<bool>,
    #[prop(default = Vec::new())] actions: Prop<Vec<LockAction>>,
    #[prop(default = "lock".to_owned())] id: String,
    on_submit: Callback<String>,
    on_action: Callback<usize>,
) -> NodeId {
    let open = create_memo(move || open.get());
    let time = create_memo(move || time.get());
    let date = create_memo(move || date.get());
    let user = create_memo(move || user.get());
    let error = create_memo(move || error.get());
    let busy = create_memo(move || busy.get());
    let actions = create_memo(move || actions.get());
    let password = create_signal(String::new());
    let submit = Rc::new(move |typed: String| {
        if busy.get_untracked() || typed.is_empty() {
            return;
        }
        password.set(String::new());
        on_submit.call(typed);
    });
    view! {
        <Modal
            open
            scrim=Scrim::Opaque
            placement=ModalPlacement::AllScreens
            level=LayerLevel::Lock
            motion=ModalMotion::Still
        >
            <EveryScreen>|slot: ScreenSlot| {
                let index = slot.index;
                let focused = create_memo(move || index == 0 && open.get() && !busy.get());
                let submit = submit.clone();
                view! {
                    <LockCard
                        time
                        date
                        user
                        error
                        busy
                        actions
                        password
                        focused
                        id={format!("{id}.{index}")}
                        on_submit={move |typed: String| submit(typed)}
                        on_action
                    />
                }
            }</EveryScreen>
        </Modal>
    }
}

#[component]
fn LockCard(
    time: Memo<String>,
    date: Memo<String>,
    user: Memo<String>,
    error: Memo<Option<String>>,
    busy: Memo<bool>,
    actions: Memo<Vec<LockAction>>,
    password: Signal<String>,
    focused: Memo<bool>,
    id: String,
    on_submit: Callback<String>,
    on_action: Callback<usize>,
) -> NodeId {
    component_accessibility(create_memo(|| {
        let mut node = beui::accesskit::Node::new(beui::accesskit::Role::Dialog);
        node.set_label("Locked");
        node
    }));
    let failed = create_memo(move || error.get().is_some());
    let checking = create_memo(move || busy.get() && error.get().is_none());
    let reason = create_memo(move || error.get().unwrap_or_default());
    let slots = create_memo(move || (0..actions.with(Vec::len)).collect::<Vec<usize>>());
    let error_id = format!("{id}.error");
    let action_id = id.clone();
    let password_id = format!("{id}.password");
    view! {
        <List spacing=GROUP_SPACING align=Align::Center @test_id={id}>
            <List spacing=SPACING align=Align::Center>
                <Text style=TextStyle::DISPLAY>{time}</Text>
                <Text color=TextColor::Muted>{date}</Text>
            </List>
            <List spacing=SPACING align=Align::Center>
                <Text style=TextStyle::TITLE>{user}</Text>
                <Frame width=FIELD_WIDTH>
                    <TextInput
                        value={password}
                        placeholder="Password"
                        label="Password"
                        password=true
                        disabled={busy}
                        focused={focused}
                        @test_id={password_id}
                        on_submit={on_submit}
                    />
                </Frame>
                <Show condition={checking}>
                    <Text style=TextStyle::CAPTION>"Checking…"</Text>
                </Show>
                <Show condition={failed}>
                    <Text
                        style=TextStyle::CAPTION
                        color=TextColor::Danger
                        @test_id={error_id.clone()}
                    >
                        {reason}
                    </Text>
                </Show>
            </List>
            <List direction=Direction::Horizontal spacing=ACTION_SPACING>
                <ForEach keys={slots}>|index: usize| {
                    let glyph = create_memo(move || {
                        actions.with(|actions| actions.get(index).map(|action| action.glyph))
                    });
                    let label = create_memo(move || {
                        actions.with(|actions| actions.get(index).map(|action| action.label.clone()).unwrap_or_default())
                    });
                    view! {
                        <Button
                            icon_only=true
                            glyph
                            label
                            @test_id={format!("{action_id}.action.{index}")}
                            on_click={move || on_action.call(index)}
                        />
                    }
                }</ForEach>
            </List>
        </List>
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
