use std::rc::Rc;
use std::time::Duration;

use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::{HourCycle, Time};
use block_editor_beui::beui::icons::{ICON_ADD, ICON_APPS, ICON_MENU, ICON_WORKSPACES};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, ReadSignal, Show, Spacer, clone, component,
    create_memo, create_signal, create_timer, now, view,
};
use block_editor_beui::beui::styled::{IconButton, MenuButton, Separator, use_theme};
use block_editor_beui::beui::unstyled::{MenuItem, PopoverHandle};
use block_editor_beui::{Media, utc_offset};
use block_shell::Workspace;

use super::calendar::{CALENDAR_WIDTH, DesktopCalendar};
use super::launcher::ProgramLauncher;
use super::notifications::{DesktopNotifications, NotificationsButton};
use super::popup::BarPopup;
use super::power::PowerMenu;
use super::sessions::Sessions;
use super::volume::VolumeButton;

const BAR_PADDING: f32 = 6.0;
const BAR_SPACING: f32 = 8.0;
const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

pub(crate) fn local_minutes(unix: Duration) -> u32 {
    let seconds = i64::try_from(unix.as_secs()).unwrap_or(0) + i64::from(utc_offset());
    let minutes = seconds.rem_euclid(SECONDS_PER_DAY) / 60;
    u32::try_from(minutes).unwrap_or(0)
}

fn wall_clock() -> ReadSignal<String> {
    let started = block_editor_beui::wall_clock();
    let anchor = now();
    let unix = move || started + now().saturating_duration_since(anchor);
    let shown = move || Time::from_minutes(local_minutes(unix())).format(HourCycle::H24);
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
pub(crate) fn DesktopBar(
    workspace: Rc<Workspace>,
    notifications: DesktopNotifications,
    launcher: ProgramLauncher,
) -> NodeId {
    let theme = use_theme();
    let clock = wall_clock();
    let editor = workspace.editor().clone();
    let power = editor.clone();
    let sounding = editor.clone();
    let levels = editor.host_value::<Media>();
    let audible =
        create_memo(clone!(levels -> move || levels.with(|levels| levels.output.is_some())));
    let dated = editor.clone();
    let launcher = move || launcher.show(true);
    let menu = move || editor.host().show_app_menu(editor.block_id());
    let sessions = Sessions::new(&workspace);
    let listed = sessions.listed.clone();
    let keys = create_memo(clone!(listed -> move || {
        listed.with(|sessions| sessions.iter().map(|session| session.profile).collect::<Vec<_>>())
    }));
    let named = listed.clone();
    let settings = sessions.settings.clone();
    let unready = create_memo(move || settings.get().is_none());
    let listing = create_memo(clone!(listed -> move || !listed.with(Vec::is_empty)));
    let creating = sessions.clone();
    let chose = move |path: Vec<usize>| {
        let chosen = path
            .first()
            .and_then(|index| listed.with_untracked(|sessions| sessions.get(*index).cloned()));
        if let Some(session) = chosen {
            sessions.open(session.profile);
        }
    };
    view! {
        <List spacing=0.0>
            <Separator />
            <Frame
                color={theme.surface.clone()}
                padding_horizontal=BAR_PADDING
                padding_vertical=BAR_PADDING
                @test_id={"desktop.bar"}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    <IconButton
                        glyph={ICON_MENU.to_owned()}
                        label="Menu"
                        @test_id={"desktop.menu"}
                        on_click={menu}
                    />
                    <IconButton
                        glyph={ICON_APPS.to_owned()}
                        label="Programs"
                        @test_id={"desktop.launcher"}
                        on_click={launcher}
                    />
                    <MenuButton
                        label="Sessions"
                        glyph={ICON_WORKSPACES.to_owned()}
                        disabled={unready}
                        @test_id={"desktop.sessions"}
                        items={view! {
                            <ForEach keys={keys}>
                                {move |profile: uuid::Uuid| {
                                    let named = named.clone();
                                    let label = create_memo(move || {
                                        named.with(|sessions| {
                                            sessions
                                                .iter()
                                                .find(|session| session.profile == profile)
                                                .map(|session| session.name.clone())
                                                .unwrap_or_default()
                                        })
                                    });
                                    view! {
                                        <MenuItem label />
                                    }
                                }}
                            </ForEach>
                            <MenuItem
                                row_test_id={"desktop.sessions.new".to_owned()}
                                label="New session"
                                glyph={ICON_ADD.to_owned()}
                                separated={listing}
                                on_click={move || creating.create()}
                            />
                        }}
                        on_select={chose}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Show condition={audible}>
                        <VolumeButton editor={sounding.clone()} levels={levels.clone()} />
                    </Show>
                    <NotificationsButton notifications />
                    <BarPopup label={clock} width=CALENDAR_WIDTH @test_id={"desktop.clock"}>
                        {move |_: PopoverHandle| view! {
                            <DesktopCalendar editor={dated.clone()} />
                        }}
                    </BarPopup>
                    <PowerMenu editor={power} />
                </List>
            </Frame>
        </List>
    }
}
