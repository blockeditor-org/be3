use block_editor_beui::linux_desktop;
use std::time::Duration;

use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::{HourCycle, Time};
use block_editor_beui::beui::icons::{
    ICON_BOXES, ICON_KEYBOARD, ICON_LAYOUT_GRID, ICON_MENU, ICON_PLUS, ICON_REFRESH_CW,
};
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{
    DisplaysWatch, EpaperWatch, MediaWatch, PowerWatch, WifiWatch, node_anchor, utc_offset,
};
use block_shell::Workspace;

use super::calendar::{CALENDAR_WIDTH, DesktopCalendar};
use super::launcher::ProgramLauncher;
use super::notifications::{DesktopNotifications, NotificationsButton};
use super::placeholders::{OpenAllButton, Placeholders};
use super::popup::BarPopup;
use super::power::PowerMenu;
use super::rotate::RotateButton;
use super::sessions::Sessions;
use super::tray::TrayItems;
use super::volume::VolumeButton;
use super::wifi::WifiButton;

const BAR_PADDING: Space = Space::Sm;
const BAR_SPACING: Space = Space::Md;
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
    let clock = create_signal(shown());
    let ticking = create_timer(move || {
        clock.set(shown());
        Some(until_next_minute())
    });
    ticking.start(until_next_minute());
    clock.read_only()
}

#[component]
pub(crate) fn DesktopBar(
    workspace: Workspace,
    notifications: DesktopNotifications,
    launcher: ProgramLauncher,
    placeholders: Placeholders,
    display: u32,
) -> NodeId {
    let clock = wall_clock();
    let editor = workspace.editor();
    let levels = editor.watch::<MediaWatch>();
    let audible = create_memo(move || levels.with(|levels| levels.output.is_some()));
    let wifi = editor.watch::<WifiWatch>();
    let wireless = create_memo(move || wifi.with(|wifi| wifi.available));
    let kept = placeholders.keys;
    let launches = placeholders.launchable;
    let waiting = create_memo(move || launches.get() && !kept.with(Vec::is_empty));
    let launchable = launcher.offered;
    let launcher = move || launcher.show(true);
    let notified = notifications.offered;
    let powered = editor.offered::<PowerWatch>();
    let epaper = editor.watch::<EpaperWatch>();
    let refresh = move || editor.act(linux_desktop::epaper::refresh);
    let displays = editor.watch::<DisplaysWatch>();
    let screen =
        create_memo(move || displays.with(|displays| displays.get(display as usize).cloned()));
    let display_settings = editor.effective_settings::<DisplaySettings>();
    let rotatable = create_memo(move || {
        display_settings.with(DisplaySettings::rotate_in_bar) && screen.with(Option::is_some)
    });
    let menu_button = NodeRef::new();
    let menu = move || {
        editor
            .host()
            .show_app_menu(editor.block_id(), node_anchor(&menu_button))
    };
    let sessions = Sessions::new(workspace);
    let listed = sessions.listed;
    let keys = create_memo(move || {
        listed.with(|sessions| {
            sessions
                .iter()
                .map(|session| session.block)
                .collect::<Vec<_>>()
        })
    });
    let settings = sessions.settings;
    let unready = create_memo(move || settings.get().is_none());
    let listing = create_memo(move || !listed.with(Vec::is_empty));
    let chose = move |path: Vec<usize>| {
        let chosen = path
            .first()
            .and_then(|index| listed.with_untracked(|sessions| sessions.get(*index).cloned()));
        if let Some(session) = chosen {
            sessions.open(session.block);
        }
    };
    view! {
        <List spacing=Space::None>
            <Separator />
            <Frame fill=ColorRole::Surface padding=BAR_PADDING @test_id={"desktop.bar"}>
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    <Button
                        icon_only=true
                        @node_ref={&menu_button}
                        glyph=ICON_MENU
                        label="Menu"
                        @test_id={"desktop.menu"}
                        on_click={menu}
                    />
                    <Show condition={launchable}>
                        <Button
                            icon_only=true
                            glyph=ICON_LAYOUT_GRID
                            label="Programs"
                            @test_id={"desktop.launcher"}
                            on_click={launcher}
                        />
                    </Show>
                    <MenuButton
                        label="Sessions"
                        glyph=ICON_BOXES
                        disabled={unready}
                        @test_id={"desktop.sessions"}
                        items={view! {
                            <ForEach keys={keys}>|block: uuid::Uuid| {
                                let label = create_memo(move || {
                                    listed.with(|sessions| {
                                        sessions
                                            .iter()
                                            .find(|session| session.block == block)
                                            .map(|session| session.name.clone())
                                            .unwrap_or_default()
                                    })
                                });
                                view! {
                                    <MenuItem label />
                                }
                            }</ForEach>
                            <MenuItem
                                row_test_id={"desktop.sessions.new".to_owned()}
                                label="New session"
                                glyph=ICON_PLUS
                                separated={listing}
                                on_click={move || sessions.create()}
                            />
                        }}
                        on_select={chose}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Show condition={waiting}>
                        <OpenAllButton placeholders />
                    </Show>
                    <TrayItems editor={editor} />
                    <Show condition={wireless}>
                        <WifiButton editor={editor} status={wifi} />
                    </Show>
                    <Show condition={audible}>
                        <VolumeButton editor={editor} levels={levels} />
                    </Show>
                    <Show condition={notified}>
                        <NotificationsButton notifications />
                    </Show>
                    <Show condition={epaper}>
                        <Button
                            icon_only=true
                            glyph=ICON_REFRESH_CW
                            label="Refresh the screen"
                            @test_id={"desktop.refresh"}
                            press_focus=false
                            on_click={refresh}
                        />
                    </Show>
                    <Show condition={rotatable}>
                        <RotateButton editor={editor} display={screen} />
                    </Show>
                    <Button
                        icon_only=true
                        glyph=ICON_KEYBOARD
                        label="On-screen keyboard"
                        @test_id={"desktop.keyboard"}
                        press_focus=false
                        on_click={toggle_keyboard}
                    />
                    <BarPopup
                        label={clock}
                        width={Size::Custom(CALENDAR_WIDTH)}
                        @test_id={"desktop.clock"}
                    >|_: PopoverHandle|
                        <DesktopCalendar editor={editor} />
                    </BarPopup>
                    <Show condition={powered}>
                        <PowerMenu editor={editor} />
                    </Show>
                </List>
            </Frame>
        </List>
    }
}
