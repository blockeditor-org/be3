use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use block_editor_beui::be_block::WORKSPACE_EDITOR;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::{HourCycle, Time};
use block_editor_beui::beui::icons::{ICON_APPS, ICON_MENU, ICON_WORKSPACES};
use block_editor_beui::beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, ReadSignal, Spacer, clone, component,
    create_memo, create_signal, create_timer, now, view,
};
use block_editor_beui::beui::styled::{Caption, IconButton, MenuButton, Separator, use_theme};
use block_editor_beui::beui::unstyled::MenuItem;
use block_editor_beui::utc_offset;
use block_shell::Workspace;

use super::sessions::sessions;

const BAR_PADDING: f32 = 6.0;
const BAR_SPACING: f32 = 8.0;
const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

fn local_minutes(unix: Duration) -> u32 {
    let seconds = i64::try_from(unix.as_secs()).unwrap_or(0) + i64::from(utc_offset());
    let minutes = seconds.rem_euclid(SECONDS_PER_DAY) / 60;
    u32::try_from(minutes).unwrap_or(0)
}

fn wall_clock() -> ReadSignal<String> {
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
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
pub(crate) fn DesktopBar(workspace: Rc<Workspace>) -> NodeId {
    let theme = use_theme();
    let clock = wall_clock();
    let editor = workspace.editor().clone();
    let launching = editor.clone();
    let launcher = move || launching.host().show_launcher(launching.block_id());
    let menu = move || editor.host().show_app_menu(editor.block_id());
    let listed = sessions(&workspace);
    let keys = create_memo(clone!(listed -> move || {
        listed.with(|sessions| sessions.iter().map(|session| session.profile).collect::<Vec<_>>())
    }));
    let named = listed.clone();
    let empty = create_memo(clone!(listed -> move || listed.with(Vec::is_empty)));
    let opening = Rc::clone(&workspace);
    let chose = move |path: Vec<usize>| {
        let chosen = path
            .first()
            .and_then(|index| listed.with_untracked(|sessions| sessions.get(*index).cloned()));
        if let Some(session) = chosen {
            opening.open_block(session.profile, WORKSPACE_EDITOR);
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
                        disabled={empty}
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
                        }}
                        on_select={chose}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Caption content={clock} @test_id={"desktop.clock"} />
                </List>
            </Frame>
        </List>
    }
}
