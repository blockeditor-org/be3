use block_editor_beui::desktop;
use std::time::Duration;

use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::{HourCycle, Time};
use block_editor_beui::beui::icons::ICON_REFRESH_CW;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{
    DisplaysWatch, EpaperWatch, MediaWatch, PowerWatch, WifiWatch, utc_offset,
};
use block_shell::Workspace;

use super::apps::{Apps, AppsButton};
use super::clock::ClockButton;
use super::notifications::DesktopNotifications;
use super::rotate::RotateButton;
use super::status::{Status, StatusButton};
use super::tray::TrayItems;
use super::windows::{DeskWindows, WindowChips};
use beui::{Align, Direction, ItemSize};

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
    apps: Apps,
    windows: DeskWindows,
    display: u32,
) -> NodeId {
    let clock = wall_clock();
    let editor = workspace.editor();
    let levels = editor.watch::<MediaWatch>();
    let audible = create_memo(move || levels.with(|levels| levels.output.is_some()));
    let wifi = editor.watch::<WifiWatch>();
    let wireless = create_memo(move || wifi.with(|wifi| wifi.available));
    let powered = editor.offered::<PowerWatch>();
    let epaper = editor.watch::<EpaperWatch>();
    let refresh = move || editor.act(desktop::epaper::refresh);
    let displays = editor.watch::<DisplaysWatch>();
    let screen =
        create_memo(move || displays.with(|displays| displays.get(display as usize).cloned()));
    let display_settings = editor.effective_settings::<DisplaySettings>();
    let rotatable = create_memo(move || {
        display_settings.with(DisplaySettings::rotate_in_bar) && screen.with(Option::is_some)
    });
    let status = Status {
        editor,
        wifi,
        wireless,
        levels,
        audible,
        powered,
        epaper,
        screen,
    };
    view! {
        <List spacing=Space::None>
            <Separator />
            <Frame fill=ColorRole::Surface padding=BAR_PADDING @test_id={"desktop.bar"}>
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    <AppsButton apps display />
                    <WindowChips @sizing=ItemSize::Percent(100.0) windows display />
                    <TrayItems editor={editor} />
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
                    <StatusButton status />
                    <ClockButton editor={editor} notifications clock />
                </List>
            </Frame>
        </List>
    }
}
