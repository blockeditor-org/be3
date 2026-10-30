use block_editor_beui::desktop;
use std::time::Duration;

use block_editor_beui::be_block::DisplaySettings;
use block_editor_beui::beui::NodeId;
use block_editor_beui::beui::datetime::{HourCycle, Time};
use block_editor_beui::be_block::ViewState;
use block_editor_beui::beui::dock::styled::{
    DockAlign, DockBarGrip, DockBarPlacement, DockBarSurface, DockEdge, DockingLayout,
    DockingSnapshot, TabId, use_dock_bar,
};
use block_editor_beui::beui::dock::unstyled::DockState;
use block_editor_beui::beui::icons::{ICON_KEYBOARD, ICON_REFRESH_CW};
use block_editor_beui::beui::reactive::toggle_keyboard;
use block_editor_beui::beui::reactive::*;
use block_editor_beui::beui::styled::*;
use block_editor_beui::{
    DisplaysWatch, Editor, EpaperWatch, MediaWatch, PowerWatch, WifiWatch, utc_offset,
};
use block_shell::{SHELL_BARS, Workspace};

use super::apps::{Apps, AppsButton};
use super::clock::ClockButton;
use super::notifications::DesktopNotifications;
use super::rotate::RotateButton;
use super::status::{Status, StatusButton};
use super::tray::TrayItems;
use super::windows::{DeskWindows, WindowChips};
use beui::{Align, Direction};

pub(crate) const APPS_PLACE: DockBarPlacement =
    DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Start);
pub(crate) const WINDOWS_PLACE: DockBarPlacement =
    DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center);
pub(crate) const STATUS_PLACE: DockBarPlacement =
    DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::End);
const PLACES: [DockBarPlacement; 3] = [APPS_PLACE, WINDOWS_PLACE, STATUS_PLACE];
const DISPLAY_BARS: &str = "display-bars";

const STATUS_SPACING: Space = Space::Xs2;
const SECONDS_PER_MINUTE: u64 = 60;
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

pub(crate) fn local_minutes(unix: Duration) -> u32 {
    let seconds = i64::try_from(unix.as_secs()).unwrap_or(0) + i64::from(utc_offset());
    let minutes = seconds.rem_euclid(SECONDS_PER_DAY) / 60;
    u32::try_from(minutes).unwrap_or(0)
}

fn wall_clock() -> ReadSignal<String> {
    let unix = block_editor_beui::wall_clock;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bars {
    pub(crate) apps: TabId,
    pub(crate) windows: TabId,
    pub(crate) status: TabId,
}

impl Bars {
    pub(crate) const fn of(display: u32) -> Self {
        let first = SHELL_BARS + 3 * display as u64;
        Self {
            apps: TabId::new(first),
            windows: TabId::new(first + 1),
            status: TabId::new(first + 2),
        }
    }

    pub(crate) fn all(self) -> [TabId; 3] {
        [self.apps, self.windows, self.status]
    }
}

pub(crate) fn display_layout(
    editor: Editor,
    first: DockingLayout<TabId>,
    display: u32,
) -> DockingLayout<TabId> {
    let layout = DockingLayout::new();
    let saved = untrack(|| saved_bars(editor))
        .into_iter()
        .find(|(held, _)| *held == display)
        .map(|(_, snapshot)| snapshot);
    let snapshot = saved.unwrap_or_else(|| untrack(|| like_the_first(first, display)));
    layout.restore(snapshot);
    create_effect(move || {
        let snapshot = layout.snapshot();
        untrack(|| save_bars(editor, display, snapshot));
    });
    layout
}

fn like_the_first(first: DockingLayout<TabId>, display: u32) -> DockingSnapshot<TabId> {
    let mut state = DockState::default();
    let bars = Bars::of(display).all();
    for ((bar, from), placement) in bars.into_iter().zip(Bars::of(0).all()).zip(PLACES) {
        state.add_bar(bar, None, first.bar_placement(&from).unwrap_or(placement));
        state.set_bar_hidden(bar, first.is_bar_hidden(&from));
    }
    DockingSnapshot::new(state, bars.map(|bar| (bar, bar)))
}

fn saved_bars(editor: Editor) -> Vec<(u32, DockingSnapshot<TabId>)> {
    editor
        .view_state(DISPLAY_BARS)
        .and_then(|state| state.value::<Vec<(u32, DockingSnapshot<TabId>)>>())
        .unwrap_or_default()
}

fn save_bars(editor: Editor, display: u32, snapshot: DockingSnapshot<TabId>) {
    let mut saved = saved_bars(editor);
    match saved.iter_mut().find(|(held, _)| *held == display) {
        Some((_, held)) if *held == snapshot => return,
        Some((_, held)) => *held = snapshot,
        None => saved.push((display, snapshot)),
    }
    saved.sort_by_key(|(held, _)| *held);
    editor.set_view_state(DISPLAY_BARS, Some(&ViewState::new(&saved, Vec::new())));
}

pub(crate) fn bar_direction() -> Memo<Direction> {
    let bar = use_dock_bar();
    create_memo(move || bar.map_or(Direction::Horizontal, |bar| bar.direction()))
}

pub(crate) fn toast_corner(placement: Option<DockBarPlacement>) -> Edge {
    let Some(placement) = placement.filter(|placement| !placement.is_window()) else {
        return Edge::BottomEnd;
    };
    match (placement.edge(), placement.align()) {
        (DockEdge::Top | DockEdge::Left, DockAlign::Start) => Edge::TopStart,
        (DockEdge::Top, DockAlign::Center) => Edge::Top,
        (DockEdge::Top, DockAlign::End) | (DockEdge::Right, DockAlign::Start) => Edge::TopEnd,
        (DockEdge::Bottom, DockAlign::Start) | (DockEdge::Left, _) => Edge::BottomStart,
        (DockEdge::Bottom, DockAlign::Center) => Edge::Bottom,
        (DockEdge::Bottom, DockAlign::End) | (DockEdge::Right, _) => Edge::BottomEnd,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Shell {
    pub(crate) workspace: Workspace,
    pub(crate) notifications: DesktopNotifications,
    pub(crate) apps: Apps,
    pub(crate) windows: DeskWindows,
    pub(crate) clock: ReadSignal<String>,
}

impl Shell {
    pub(crate) fn new(
        workspace: Workspace,
        notifications: DesktopNotifications,
        apps: Apps,
        windows: DeskWindows,
    ) -> Self {
        Self {
            workspace,
            notifications,
            apps,
            windows,
            clock: wall_clock(),
        }
    }
}

#[component]
pub(crate) fn AppsBar(shell: Shell, display: u32, layout: DockingLayout<TabId>) -> NodeId {
    let bar = use_dock_bar();
    let shown = create_memo(move || bar.is_none_or(|bar| !bar.is_hidden()));
    view! {
        <DockBarSurface @test_id={"desktop.bar.apps"}>
            <DockBarGrip />
            <Show condition={shown}>
                <AppsButton apps={shell.apps} display layout />
            </Show>
        </DockBarSurface>
    }
}

#[component]
pub(crate) fn WindowsBar(shell: Shell, display: u32) -> NodeId {
    let direction = bar_direction();
    view! {
        <DockBarSurface @test_id={"desktop.bar.windows"}>
            <DockBarGrip />
            <WindowChips windows={shell.windows} direction display />
        </DockBarSurface>
    }
}

#[component]
pub(crate) fn StatusBar(shell: Shell, display: u32) -> NodeId {
    view! {
        <DockBarSurface @test_id={"desktop.bar.status"}>
            <DockBarGrip />
            <StatusItems shell display />
        </DockBarSurface>
    }
}

#[component]
fn StatusItems(shell: Shell, display: u32) -> NodeId {
    let direction = bar_direction();
    let editor = shell.workspace.editor();
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
    let keyboard = create_memo(move || display_settings.with(DisplaySettings::keyboard_in_bar));
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
        <List direction align=Align::Center spacing=STATUS_SPACING>
            <TrayItems editor={editor} direction />
            <Show condition={keyboard}>
                <Button
                    icon_only=true
                    glyph=ICON_KEYBOARD
                    label="On-screen keyboard"
                    variant=ButtonVariant::Ghost
                    @test_id={"desktop.keyboard"}
                    press_focus=false
                    on_click={toggle_keyboard}
                />
            </Show>
            <Show condition={rotatable}>
                <RotateButton editor={editor} display={screen} />
            </Show>
            <Show condition={epaper}>
                <Button
                    icon_only=true
                    glyph=ICON_REFRESH_CW
                    label="Refresh the screen"
                    variant=ButtonVariant::Ghost
                    @test_id={"desktop.refresh"}
                    press_focus=false
                    on_click={refresh}
                />
            </Show>
            <StatusButton status direction />
            <ClockButton
                editor={editor}
                notifications={shell.notifications}
                clock={shell.clock}
                direction
            />
        </List>
    }
}
