use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use block_editor_beui::beui::datetime::{HourCycle, Time};
use block_editor_beui::beui::icons::ICON_APPS;
use block_editor_beui::beui::reactive::{
    Align, Direction, Frame, ItemSize, List, ReadSignal, Spacer, component, create_signal,
    create_timer, now, view,
};
use block_editor_beui::beui::styled::{Caption, IconButton, Separator, use_theme};
use block_editor_beui::beui::{NodeId, Rect, Vec2, pos2, vec2};
use block_editor_beui::utc_offset;

use super::workspace::Workspace;

const BAR_PADDING: f32 = 6.0;
const BAR_SPACING: f32 = 8.0;
const WINDOW_MARGIN: f32 = 48.0;
const WINDOW_SIZE: Vec2 = vec2(1100.0, 720.0);
const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

pub(crate) fn workspace_window(area: Vec2) -> Rect {
    let fits = area - vec2(WINDOW_MARGIN * 2.0, WINDOW_MARGIN * 3.0);
    let size = match fits.x > 0.0 && fits.y > 0.0 {
        true => fits,
        false => WINDOW_SIZE,
    };
    Rect::from_min_size(pos2(WINDOW_MARGIN, WINDOW_MARGIN), size)
}

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
    let menu = move || editor.host().show_app_menu(editor.block_id());
    view! {
        <List spacing=0.0>
            <Separator />
            <Frame
                color={theme.surface.clone()}
                padding_horizontal=BAR_PADDING
                padding_vertical=BAR_PADDING
                @test_id={"workspace.desktop-bar"}
            >
                <List direction=Direction::Horizontal align=Align::Center spacing=BAR_SPACING>
                    <IconButton
                        glyph={ICON_APPS.to_owned()}
                        label="Menu"
                        @test_id={"workspace.desktop-menu"}
                        on_click={menu}
                    />
                    <Spacer @sizing=ItemSize::Percent(100.0) />
                    <Caption content={clock} @test_id={"workspace.clock"} />
                </List>
            </Frame>
        </List>
    }
}
