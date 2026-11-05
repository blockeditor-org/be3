use be_sdk_beui::desktop;
use std::time::Duration;

use be_sdk_beui::be_block::DisplaySettings;
use be_sdk_beui::be_block::ViewState;
use be_sdk_beui::beui::datetime::{HourCycle, SECONDS_PER_DAY, Time};
use be_sdk_beui::beui::dock::DockState;
use be_sdk_beui::beui::dock::styled::{DockBarGrip, DockBarSurface};
use be_sdk_beui::beui::dock::{
    DockAlign, DockBarPlacement, DockEdge, DockingLayout, DockingSnapshot, TabId, use_dock_bar,
};
use be_sdk_beui::beui::dock_host::styled::DockHostLayout;
use be_sdk_beui::beui::icons::{ICON_KEYBOARD, ICON_REFRESH_CW};
use be_sdk_beui::beui::reactive::toggle_keyboard;
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{NodeId, Rect};
use be_sdk_beui::{
    DisplaysWatch, Editor, EpaperWatch, MediaWatch, PowerWatch, SavedDockLayout, WifiWatch,
    utc_offset,
};
use be_sdk_shell::{SHELL_BARS, Workspace};

use super::clock::ClockButton;
use super::layouts::{LayoutButton, Layouts};
use super::notifications::DesktopNotifications;
use super::programs_panel::{ProgramsPanel, ProgramsPanelButton};
use super::rotate::RotateButton;
use super::status::{Status, StatusButton};
use super::tray::TrayItems;
use super::windows::{DeskWindows, WindowChips};
use beui::{Align, Direction};

pub(crate) const PROGRAMS_PLACE: DockBarPlacement =
    DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Start);
pub(crate) const WINDOWS_PLACE: DockBarPlacement =
    DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::Center);
pub(crate) const STATUS_PLACE: DockBarPlacement =
    DockBarPlacement::Beside(DockEdge::Bottom, DockAlign::End);
const PLACES: [DockBarPlacement; 3] = [PROGRAMS_PLACE, WINDOWS_PLACE, STATUS_PLACE];
const DISPLAY_BARS: &str = "display-bars-by-monitor";

const STATUS_SPACING: Space = Space::Xs2;
const SECONDS_PER_MINUTE: u64 = 60;

pub(crate) fn local_minutes(unix: Duration) -> u32 {
    let seconds = i64::try_from(unix.as_secs()).unwrap_or(0) + i64::from(utc_offset());
    let minutes = seconds.rem_euclid(SECONDS_PER_DAY) / 60;
    u32::try_from(minutes).unwrap_or(0)
}

fn wall_clock() -> ReadSignal<String> {
    let unix = be_sdk_beui::wall_clock;
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
    pub(crate) programs: TabId,
    pub(crate) windows: TabId,
    pub(crate) status: TabId,
}

impl Bars {
    pub(crate) const fn of(display: u32) -> Self {
        let first = SHELL_BARS + 3 * display as u64;
        Self {
            programs: TabId::new(first),
            windows: TabId::new(first + 1),
            status: TabId::new(first + 2),
        }
    }

    pub(crate) fn all(self) -> [TabId; 3] {
        [self.programs, self.windows, self.status]
    }

    fn as_bar_of(self, tab: TabId, display: u32) -> TabId {
        self.all()
            .into_iter()
            .zip(Self::of(display).all())
            .find(|(own, _)| *own == tab)
            .map_or(tab, |(_, other)| other)
    }
}

pub(crate) fn display_layout(
    editor: Editor,
    first: DockingLayout<TabId>,
    display: u32,
) -> DockingLayout<TabId> {
    let layout = DockingLayout::new();
    let displays = editor.watch::<DisplaysWatch>();
    let monitor = create_memo(move || {
        displays.with(|displays| displays.get(display as usize).map(|shown| shown.id.clone()))
    });
    let showing = create_signal(None::<String>);
    let show = move |monitor: Option<String>| {
        showing.set(None);
        let saved = monitor.as_deref().and_then(|monitor| {
            untrack(|| saved_bars(editor))
                .ok()?
                .into_iter()
                .find(|(held, _)| held == monitor)
                .map(|(_, saved)| {
                    saved
                        .map_tabs(|tab| Bars::of(0).as_bar_of(tab, display))
                        .snapshot()
                })
        });
        layout.restore(saved.unwrap_or_else(|| untrack(|| like_the_first(first, display))));
        showing.set(monitor);
    };
    show(monitor.get_untracked());
    create_effect(move || {
        let monitor = monitor.get();
        if untrack(|| showing.get()) != monitor {
            untrack(|| show(monitor));
        }
    });
    let readable = create_memo(move || saved_bars(editor).is_ok());
    create_effect(move || {
        let saved = SavedDockLayout::of(&layout.snapshot())
            .map_tabs(|tab| Bars::of(display).as_bar_of(tab, 0));
        let Some(monitor) = showing.get().filter(|_| readable.get()) else {
            return;
        };
        untrack(|| save_bars(editor, monitor, saved));
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
    for edge in DockEdge::ALL {
        state.set_edge_auto_hidden(edge, first.is_edge_auto_hidden(edge));
    }
    DockingSnapshot::new(state, bars.map(|bar| (bar, bar)))
}

type SavedBars = Vec<(String, SavedDockLayout)>;

fn saved_bars(editor: Editor) -> Result<SavedBars, String> {
    let Some(state) = editor.view_state(DISPLAY_BARS) else {
        return Ok(Vec::new());
    };
    state.read::<SavedBars>().map_err(|error| {
        format!(
            "The saved bars of the further displays of session {} could not be read: {error}\n\nThey hold {} bytes, kept unchanged until the bars are reset.",
            editor.block_id(),
            state.bytes.len()
        )
    })
}

fn save_bars(editor: Editor, monitor: String, layout: SavedDockLayout) {
    let Ok(mut saved) = saved_bars(editor) else {
        return;
    };
    match saved.iter_mut().find(|(held, _)| *held == monitor) {
        Some((_, held)) if *held == layout => return,
        Some((_, held)) => *held = layout,
        None => saved.push((monitor, layout)),
    }
    saved.sort_by(|(left, _), (right, _)| left.cmp(right));
    editor.set_view_state(DISPLAY_BARS, Some(&ViewState::new(&saved, Vec::new())));
}

#[component]
pub(crate) fn UnreadableBarsDialog(editor: Editor) -> NodeId {
    let report = create_memo(move || saved_bars(editor).err());
    let displays = editor.watch::<DisplaysWatch>();
    let further = create_memo(move || displays.with(|displays| displays.len() > 1));
    let kept = create_signal(false);
    let open = create_memo(move || report.with(Option::is_some) && further.get() && !kept.get());
    let reset = move || {
        let empty: SavedBars = Vec::new();
        editor.set_view_state(DISPLAY_BARS, Some(&ViewState::new(&empty, Vec::new())));
    };
    view! {
        <Dialog
            open={open}
            kind=DialogKind::Alert
            title="The bars of the other displays could not be read"
            width=Size::Dialog
            id="desktop.display-bars"
            actions={vec![
                DialogAction::secondary("Copy report", move || {
                    if let Some(report) = report.get_untracked() {
                        copy(Clip::text(report));
                    }
                })
                .named("copy-report"),
                DialogAction::secondary("Keep them", move || kept.set(true)).named("keep"),
                DialogAction::destructive("Reset bars", reset).named("reset"),
            ]}
            on_dismiss={move || kept.set(true)}
        >
            <Text style=TextStyle::PARAGRAPH>
                "Where each display other than the first keeps its bars was saved in a form this version cannot read, so they start where the first display's are."
            </Text>
            <Text style=TextStyle::PARAGRAPH>
                "Reset bars forgets the saved places and saves new ones from now on. Keep them leaves the saved places as they are, and moving a bar on another display is not saved until they are reset."
            </Text>
        </Dialog>
    }
}

#[derive(Clone, Copy)]
pub(crate) struct BarArea(pub(crate) Memo<Rect>);

pub(crate) fn bar_bounds() -> Memo<Option<Rect>> {
    let bar = use_dock_bar();
    let area = use_context::<BarArea>();
    create_memo(move || match (bar, area) {
        (Some(bar), Some(BarArea(area))) if !bar.placement().is_window() => Some(area.get()),
        _ => None,
    })
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

pub(crate) fn clear_of_status(
    layout: DockHostLayout,
    status: Option<DockBarPlacement>,
    area: Rect,
) -> Rect {
    let Some(DockBarPlacement::Over(edge, align)) = status else {
        return area;
    };
    let Some(over) = layout.over_bars_rect(edge, align) else {
        return area;
    };
    let mut clear = area;
    match edge {
        DockEdge::Top => clear.min.y = clear.min.y.max(over.bottom()),
        DockEdge::Bottom => clear.max.y = clear.max.y.min(over.top()),
        DockEdge::Left => clear.min.x = clear.min.x.max(over.right()),
        DockEdge::Right => clear.max.x = clear.max.x.min(over.left()),
    }
    match clear.is_positive() {
        true => clear,
        false => area,
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Shell {
    pub(crate) workspace: Workspace,
    pub(crate) notifications: DesktopNotifications,
    pub(crate) programs_panel: ProgramsPanel,
    pub(crate) windows: DeskWindows,
    pub(crate) clock: ReadSignal<String>,
    pub(crate) layouts: Layouts,
    pub(crate) edges: super::edges::HiddenEdges,
}

impl Shell {
    pub(crate) fn new(
        workspace: Workspace,
        notifications: DesktopNotifications,
        programs_panel: ProgramsPanel,
        windows: DeskWindows,
        layouts: Layouts,
    ) -> Self {
        Self {
            workspace,
            notifications,
            programs_panel,
            windows,
            clock: wall_clock(),
            layouts,
            edges: super::edges::HiddenEdges::new(workspace.editor()),
        }
    }
}

#[component]
pub(crate) fn ProgramsBar(shell: Shell, display: u32, layout: DockingLayout<TabId>) -> NodeId {
    let bar = use_dock_bar();
    let shown = create_memo(move || bar.is_none_or(|bar| !bar.is_hidden()));
    view! {
        <DockBarSurface @test_id={"desktop.bar.apps"}>
            <DockBarGrip bounds={bar_bounds()} />
            <Show condition={shown}>
                <ProgramsPanelButton programs_panel={shell.programs_panel} display layout />
            </Show>
        </DockBarSurface>
    }
}

#[component]
pub(crate) fn WindowsBar(shell: Shell, display: u32) -> NodeId {
    let direction = bar_direction();
    view! {
        <DockBarSurface @test_id={"desktop.bar.windows"}>
            <DockBarGrip bounds={bar_bounds()} />
            <ListItem shrink=1.0>
                <WindowChips windows={shell.windows} direction display />
            </ListItem>
        </DockBarSurface>
    }
}

#[component]
pub(crate) fn StatusBar(shell: Shell, display: u32) -> NodeId {
    view! {
        <DockBarSurface @test_id={"desktop.bar.status"}>
            <DockBarGrip bounds={bar_bounds()} />
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
    let refresh = move || editor.act(|| desktop::epaper::refresh().detach());
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
            <Show condition={shell.layouts.several}>
                <LayoutButton layouts={shell.layouts} />
            </Show>
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
                workspace={shell.workspace}
                notifications={shell.notifications}
                clock={shell.clock}
                direction
            />
        </List>
    }
}
