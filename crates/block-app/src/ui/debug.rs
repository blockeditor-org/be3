use beui::icons::{
    ICON_ARROW_BACK, ICON_ARROW_FORWARD, ICON_PAUSE, ICON_PLAY_ARROW, ICON_SKIP_NEXT,
};
use beui::reactive::{
    Align, ClickCatcher, Direction, Focusable, ForEach, Frame, ItemSize, List, Memo, Show, Spacer,
    Text, VirtualList, clone, component, component_size, create_effect, create_memo, layout_text,
    untrack, view,
};
use beui::styled::{
    Button, ButtonVariant, Caption, Code, Heading, Icon, IconButton, Link, Scroll, Spinner, Window,
    use_theme,
};
use beui::{
    Color32, FontId, Key, KeyPress, Modifiers, NodeId, ScrollGesture, TextLayout, pos2, vec2,
};

use super::onboarding::ErrorText;
use super::{AppViewStore, UiCommand, send};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DebugWindow {
    Client,
    Network,
    Performance,
    Plugins,
    Version,
    Terminal,
}

#[cfg_attr(not(feature = "terminal"), allow(dead_code))]
#[derive(Clone, Debug)]
pub(crate) enum TerminalInput {
    Key(Key, Modifiers),
    Text(String),
    Scroll(isize),
    Resize {
        cols: u16,
        rows: u16,
        cell_width: u32,
        cell_height: u32,
    },
    Retry,
}

#[cfg_attr(not(feature = "terminal"), allow(dead_code))]
#[derive(Clone, Debug)]
pub(crate) enum DebugCommand {
    Open(DebugWindow),
    Close(DebugWindow),
    PauseSending,
    StepSending,
    ResumeSending,
    ClearTraffic,
    KillPlugin(String),
    RefreshVersions,
    Install(u64),
    OpenUrl(String),
    Terminal(TerminalInput),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LineStyle {
    Heading,
    Body,
    Code,
    Muted,
    Error,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Line {
    pub(crate) text: String,
    pub(crate) style: LineStyle,
    pub(crate) indent: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TrafficRow {
    pub(crate) index: usize,
    pub(crate) sent: bool,
    pub(crate) timestamp: String,
    pub(crate) payload: String,
    pub(crate) decoded: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct NetworkView {
    pub(crate) paused: bool,
    pub(crate) queued: usize,
    pub(crate) entries: Vec<TrafficRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PerformanceRow {
    pub(crate) heading: bool,
    pub(crate) name: String,
    pub(crate) current: String,
    pub(crate) average: String,
    pub(crate) peak: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RuntimeView {
    pub(crate) id: String,
    pub(crate) state: String,
    pub(crate) lines: Vec<Line>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct PluginsView {
    pub(crate) lines: Vec<Line>,
    pub(crate) runtimes: Vec<RuntimeView>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunView {
    pub(crate) id: u64,
    pub(crate) number: u64,
    pub(crate) branch: String,
    pub(crate) event: String,
    pub(crate) sha: String,
    pub(crate) created: String,
    pub(crate) status: String,
    pub(crate) url: String,
    pub(crate) current: bool,
    pub(crate) succeeded: bool,
    pub(crate) installing: bool,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum VersionRuns {
    Loading,
    Failed(String),
    Loaded(Vec<RunView>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VersionView {
    pub(crate) commit: String,
    pub(crate) can_install: bool,
    pub(crate) runs: VersionRuns,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TerminalSpan {
    pub(crate) text: String,
    pub(crate) color: Color32,
    pub(crate) background: Option<Color32>,
    pub(crate) bold: bool,
    pub(crate) italic: bool,
    pub(crate) underline: bool,
}

impl TerminalSpan {
    #[cfg_attr(not(feature = "terminal"), allow(dead_code))]
    pub(crate) fn same_style(&self, other: &Self) -> bool {
        self.color == other.color
            && self.background == other.background
            && self.bold == other.bold
            && self.italic == other.italic
            && self.underline == other.underline
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TerminalRow {
    pub(crate) spans: Vec<TerminalSpan>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TerminalView {
    pub(crate) rows: Vec<TerminalRow>,
    pub(crate) background: Color32,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DebugView {
    pub(crate) client: Option<Vec<Line>>,
    pub(crate) network: Option<NetworkView>,
    pub(crate) performance: Option<Vec<PerformanceRow>>,
    pub(crate) plugins: Option<PluginsView>,
    pub(crate) version: Option<VersionView>,
    pub(crate) terminal: Option<TerminalView>,
}

fn debug(command: DebugCommand) {
    send(UiCommand::Debug(command));
}

const LINE_HEIGHT: f32 = 18.0;
const INDENT: f32 = 14.0;
const TERMINAL_FONT_SIZE: f32 = 13.0;
const TERMINAL_PADDING: f32 = 6.0;

#[component]
pub(super) fn DebugWindows(view: AppViewStore) -> NodeId {
    let debug = view.debug.clone();
    let client = create_memo(clone!(debug -> move || debug.get().client));
    let network = create_memo(clone!(debug -> move || debug.get().network));
    let performance = create_memo(clone!(debug -> move || debug.get().performance));
    let plugins = create_memo(clone!(debug -> move || debug.get().plugins));
    let version = create_memo(clone!(debug -> move || debug.get().version));
    let terminal = create_memo(move || debug.get().terminal);
    view! {
        <List spacing=0.0>
            <ClientWindow client />
            <NetworkWindow network />
            <PerformanceWindow performance />
            <PluginsWindow plugins />
            <VersionWindow version />
            <TerminalWindow terminal />
        </List>
    }
}

#[component]
fn LinesView(lines: Memo<Vec<Line>>) -> NodeId {
    let keys = create_memo(clone!(lines -> move || (0..lines.get().len()).collect::<Vec<_>>()));
    view! {
        <Scroll>
            <VirtualList keys={keys} item_size=LINE_HEIGHT>
                {move |index: usize| {
                    let lines = lines.clone();
                    let line = create_memo(move || lines.get().get(index).cloned());
                    view! {
                        <LineView line />
                    }
                }}
            </VirtualList>
        </Scroll>
    }
}

#[component]
fn LineView(line: Memo<Option<Line>>) -> NodeId {
    let theme = use_theme();
    let indent = create_memo(clone!(line -> move || {
        ItemSize::Fixed(line.get().map_or(0.0, |line| f32::from(line.indent) * INDENT))
    }));
    let text =
        create_memo(clone!(line -> move || line.get().map(|line| line.text).unwrap_or_default()));
    let style =
        create_memo(clone!(line -> move || line.get().map_or(LineStyle::Body, |line| line.style)));
    let color = create_memo(clone!(theme style -> move || match style.get() {
        LineStyle::Heading | LineStyle::Body | LineStyle::Code => theme.text.get(),
        LineStyle::Muted => theme.text_muted.get(),
        LineStyle::Error => theme.danger.get(),
    }));
    let bold = create_memo(clone!(style -> move || style.get() == LineStyle::Heading));
    let monospace = create_memo(clone!(style -> move || style.get() == LineStyle::Code));
    let size = create_memo(move || match style.get() {
        LineStyle::Heading => 14.0,
        _ => 12.0,
    });
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <Spacer @sizing={indent} />
            <Text
                @sizing=ItemSize::Percent(100.0)
                string={text}
                font_size={size}
                color={color}
                bold
                monospace
                wrap=true
            />
        </List>
    }
}

#[component]
fn ClientWindow(client: Memo<Option<Vec<Line>>>) -> NodeId {
    let open = create_memo(clone!(client -> move || client.get().is_some()));
    let lines = create_memo(move || client.get().unwrap_or_default());
    view! {
        <Window
            open={open}
            title="Block Client State"
            position={pos2(60.0, 60.0)}
            size={vec2(760.0, 600.0)}
            on_close={|| debug(DebugCommand::Close(DebugWindow::Client))}
        >
            <LinesView lines />
        </Window>
    }
}

#[component]
fn NetworkWindow(network: Memo<Option<NetworkView>>) -> NodeId {
    let open = create_memo(clone!(network -> move || network.get().is_some()));
    let paused =
        create_memo(clone!(network -> move || network.get().is_some_and(|network| network.paused)));
    let sending = create_memo(clone!(paused -> move || !paused.get()));
    let queued =
        create_memo(clone!(network -> move || network.get().map_or(0, |network| network.queued)));
    let cannot_step =
        create_memo(clone!(paused queued -> move || !paused.get() || queued.get() == 0));
    let summary = create_memo(clone!(paused queued -> move || match paused.get() {
        true => format!("Paused, {} queued", queued.get()),
        false => format!("Sending, {} queued", queued.get()),
    }));
    let entries = create_memo(move || {
        network
            .get()
            .map(|network| network.entries)
            .unwrap_or_default()
    });
    let keys = create_memo(clone!(entries -> move || {
        entries.get().into_iter().map(|entry| entry.index).collect::<Vec<_>>()
    }));
    let empty = create_memo(clone!(keys -> move || keys.get().is_empty()));
    view! {
        <Window
            open={open}
            title="Network Traffic"
            position={pos2(90.0, 80.0)}
            size={vec2(720.0, 480.0)}
            on_close={|| debug(DebugCommand::Close(DebugWindow::Network))}
        >
            <List spacing=8.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=6.0>
                    <IconButton
                        glyph={ICON_PAUSE.to_owned()}
                        label="Pause sending"
                        disabled={paused.clone()}
                        on_click={|| debug(DebugCommand::PauseSending)}
                    />
                    <IconButton
                        glyph={ICON_SKIP_NEXT.to_owned()}
                        label="Send the next queued message"
                        disabled={cannot_step}
                        on_click={|| debug(DebugCommand::StepSending)}
                    />
                    <IconButton
                        glyph={ICON_PLAY_ARROW.to_owned()}
                        label="Resume sending"
                        disabled={sending}
                        on_click={|| debug(DebugCommand::ResumeSending)}
                    />
                    <Button
                        label="Clear"
                        variant=ButtonVariant::Secondary
                        on_click={|| debug(DebugCommand::ClearTraffic)}
                    />
                    <Caption content={summary} />
                </List>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <Show condition={empty}>
                        <Caption content="No network traffic yet" />
                    </Show>
                    <VirtualList keys={keys} item_size=64.0>
                        {move |index: usize| {
                            let entries = entries.clone();
                            let entry = create_memo(move || {
                                entries.get().into_iter().find(|entry| entry.index == index)
                            });
                            view! {
                                <TrafficEntry entry />
                            }
                        }}
                    </VirtualList>
                </Scroll>
            </List>
        </Window>
    }
}

#[component]
fn TrafficEntry(entry: Memo<Option<TrafficRow>>) -> NodeId {
    let theme = use_theme();
    let sent = create_memo(clone!(entry -> move || entry.get().is_some_and(|entry| entry.sent)));
    let glyph = create_memo(clone!(sent -> move || match sent.get() {
        true => ICON_ARROW_FORWARD.to_owned(),
        false => ICON_ARROW_BACK.to_owned(),
    }));
    let color = create_memo(clone!(theme sent -> move || match sent.get() {
        true => theme.accent.get(),
        false => theme.warning.get(),
    }));
    let timestamp = create_memo(
        clone!(entry -> move || entry.get().map(|entry| entry.timestamp).unwrap_or_default()),
    );
    let payload = create_memo(
        clone!(entry -> move || entry.get().map(|entry| entry.payload).unwrap_or_default()),
    );
    let decoded = create_memo(move || {
        entry
            .get()
            .map(|entry| entry.decoded.join("\n"))
            .unwrap_or_default()
    });
    let has_decoded = create_memo(clone!(decoded -> move || !decoded.get().is_empty()));
    view! {
        <Frame padding_vertical=3.0>
            <List spacing=2.0>
                <List direction=Direction::Horizontal spacing=6.0>
                    <Icon glyph={glyph} color={color} />
                    <Caption content={timestamp} />
                    <Text
                        @sizing=ItemSize::Percent(100.0)
                        string={payload}
                        font_size=12.0
                        color={theme.text.clone()}
                        monospace=true
                        wrap=true
                    />
                </List>
                <Show condition={has_decoded}>
                    <Frame padding_horizontal=28.0>
                        <Text
                            string={decoded}
                            font_size=12.0
                            color={theme.text_muted.clone()}
                            monospace=true
                            wrap=true
                        />
                    </Frame>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn PerformanceWindow(performance: Memo<Option<Vec<PerformanceRow>>>) -> NodeId {
    let open = create_memo(clone!(performance -> move || performance.get().is_some()));
    let rows = create_memo(move || performance.get().unwrap_or_default());
    let keys = create_memo(clone!(rows -> move || (0..rows.get().len()).collect::<Vec<_>>()));
    view! {
        <Window
            open={open}
            title="Performance"
            position={pos2(120.0, 100.0)}
            size={vec2(520.0, 420.0)}
            on_close={|| debug(DebugCommand::Close(DebugWindow::Performance))}
        >
            <Scroll>
                <PerformanceLine
                    row={create_memo(|| Some(PerformanceRow {
                        heading: true,
                        name: "Measurement".to_owned(),
                        current: "Current".to_owned(),
                        average: "Average".to_owned(),
                        peak: "Peak".to_owned(),
                    }))}
                />
                <ForEach keys={keys}>
                    {move |index: usize| {
                        let rows = rows.clone();
                        let row = create_memo(move || rows.get().get(index).cloned());
                        view! {
                            <PerformanceLine row />
                        }
                    }}
                </ForEach>
            </Scroll>
        </Window>
    }
}

#[component]
fn PerformanceLine(row: Memo<Option<PerformanceRow>>) -> NodeId {
    let theme = use_theme();
    let cell = |read: fn(&PerformanceRow) -> String| {
        let row = row.clone();
        create_memo(move || row.get().map(|row| read(&row)).unwrap_or_default())
    };
    let name = cell(|row| row.name.clone());
    let current = cell(|row| row.current.clone());
    let average = cell(|row| row.average.clone());
    let peak = cell(|row| row.peak.clone());
    let bold = create_memo(move || row.get().is_some_and(|row| row.heading));
    view! {
        <List direction=Direction::Horizontal spacing=8.0>
            <Text
                @sizing=ItemSize::Percent(100.0)
                string={name}
                font_size=12.0
                color={theme.text.clone()}
                bold={bold.clone()}
            />
            <Text
                @sizing=ItemSize::Fixed(90.0)
                string={current}
                font_size=12.0
                color={theme.text.clone()}
                monospace=true
                bold={bold.clone()}
            />
            <Text
                @sizing=ItemSize::Fixed(90.0)
                string={average}
                font_size=12.0
                color={theme.text.clone()}
                monospace=true
                bold={bold.clone()}
            />
            <Text
                @sizing=ItemSize::Fixed(90.0)
                string={peak}
                font_size=12.0
                color={theme.text.clone()}
                monospace=true
                bold
            />
        </List>
    }
}

#[component]
fn PluginsWindow(plugins: Memo<Option<PluginsView>>) -> NodeId {
    let open = create_memo(clone!(plugins -> move || plugins.get().is_some()));
    let lines = create_memo(
        clone!(plugins -> move || plugins.get().map(|plugins| plugins.lines).unwrap_or_default()),
    );
    let line_keys =
        create_memo(clone!(lines -> move || (0..lines.get().len()).collect::<Vec<_>>()));
    let runtimes = create_memo(move || {
        plugins
            .get()
            .map(|plugins| plugins.runtimes)
            .unwrap_or_default()
    });
    let runtime_keys = create_memo(clone!(runtimes -> move || {
        runtimes.get().into_iter().map(|runtime| runtime.id).collect::<Vec<_>>()
    }));
    let idle = create_memo(clone!(runtime_keys -> move || runtime_keys.get().is_empty()));
    let running = create_memo(
        clone!(runtime_keys -> move || format!("Running ({})", runtime_keys.get().len())),
    );
    view! {
        <Window
            open={open}
            title="Plugins"
            position={pos2(150.0, 90.0)}
            size={vec2(560.0, 440.0)}
            on_close={|| debug(DebugCommand::Close(DebugWindow::Plugins))}
        >
            <Scroll>
                <ForEach keys={line_keys}>
                    {move |index: usize| {
                        let lines = lines.clone();
                        let line = create_memo(move || lines.get().get(index).cloned());
                        view! {
                            <LineView line />
                        }
                    }}
                </ForEach>
                <Heading content={running} />
                <Show condition={idle}>
                    <Caption content="no editor plugins are running" />
                </Show>
                <ForEach keys={runtime_keys}>
                    {move |id: String| {
                        let runtimes = runtimes.clone();
                        let key = id.clone();
                        let runtime = create_memo(move || {
                            runtimes.get().into_iter().find(|runtime| runtime.id == key)
                        });
                        view! {
                            <RuntimeBlock id runtime />
                        }
                    }}
                </ForEach>
            </Scroll>
        </Window>
    }
}

#[component]
fn RuntimeBlock(id: String, runtime: Memo<Option<RuntimeView>>) -> NodeId {
    let state = create_memo(
        clone!(runtime -> move || runtime.get().map(|runtime| runtime.state).unwrap_or_default()),
    );
    let lines = create_memo(move || {
        runtime
            .get()
            .map(|runtime| runtime.lines)
            .unwrap_or_default()
    });
    let keys = create_memo(clone!(lines -> move || (0..lines.get().len()).collect::<Vec<_>>()));
    let kill = id.clone();
    view! {
        <Frame padding_vertical=6.0>
            <List spacing=2.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Heading content={id} />
                    <Caption @sizing=ItemSize::Percent(100.0) content={state} />
                    <Button
                        label="Kill"
                        variant=ButtonVariant::Secondary
                        on_click={move || debug(DebugCommand::KillPlugin(kill.clone()))}
                    />
                </List>
                <ForEach keys={keys}>
                    {move |index: usize| {
                        let lines = lines.clone();
                        let line = create_memo(move || lines.get().get(index).cloned());
                        view! {
                            <LineView line />
                        }
                    }}
                </ForEach>
            </List>
        </Frame>
    }
}

#[component]
fn VersionWindow(version: Memo<Option<VersionView>>) -> NodeId {
    let open = create_memo(clone!(version -> move || version.get().is_some()));
    let commit = create_memo(
        clone!(version -> move || version.get().map(|version| version.commit).unwrap_or_default()),
    );
    let can_install = create_memo(
        clone!(version -> move || version.get().is_some_and(|version| version.can_install)),
    );
    let runs = create_memo(move || version.get().map(|version| version.runs));
    let loading =
        create_memo(clone!(runs -> move || matches!(runs.get(), Some(VersionRuns::Loading))));
    let error = create_memo(clone!(runs -> move || match runs.get() {
        Some(VersionRuns::Failed(error)) => Some(error),
        _ => None,
    }));
    let loaded = create_memo(clone!(runs -> move || match runs.get() {
        Some(VersionRuns::Loaded(runs)) => runs,
        _ => Vec::new(),
    }));
    let none = create_memo(
        clone!(runs -> move || matches!(runs.get(), Some(VersionRuns::Loaded(runs)) if runs.is_empty())),
    );
    let keys = create_memo(
        clone!(loaded -> move || loaded.get().into_iter().map(|run| run.id).collect::<Vec<_>>()),
    );
    view! {
        <Window
            open={open}
            title="App Version"
            position={pos2(180.0, 110.0)}
            size={vec2(640.0, 480.0)}
            on_close={|| debug(DebugCommand::Close(DebugWindow::Version))}
        >
            <List spacing=8.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Caption content="Running commit:" />
                    <Code @sizing=ItemSize::Percent(100.0) content={commit} />
                    <Button
                        label="Refresh"
                        variant=ButtonVariant::Secondary
                        on_click={|| debug(DebugCommand::RefreshVersions)}
                    />
                </List>
                <Show condition={loading}>
                    <Spinner />
                </Show>
                <ErrorText text={error} />
                <Show condition={none}>
                    <Caption content="No workflow runs found." />
                </Show>
                <Scroll @sizing=ItemSize::Percent(100.0)>
                    <ForEach keys={keys}>
                        {move |id: u64| {
                            let loaded = loaded.clone();
                            let run = create_memo(move || loaded.get().into_iter().find(|run| run.id == id));
                            view! {
                                <RunCard run can_install={can_install.clone()} />
                            }
                        }}
                    </ForEach>
                </Scroll>
            </List>
        </Window>
    }
}

#[component]
fn RunCard(run: Memo<Option<RunView>>, can_install: Memo<bool>) -> NodeId {
    let theme = use_theme();
    let field = |read: fn(&RunView) -> String| {
        let run = run.clone();
        create_memo(move || run.get().map(|run| read(&run)).unwrap_or_default())
    };
    let title = field(|run| format!("#{} {} ({})", run.number, run.branch, run.event));
    let details = field(|run| format!("{} · {} · {}", run.sha, run.created, run.status));
    let url = field(|run| run.url.clone());
    let install_label = field(|run| match run.installing {
        true => "Installing…".to_owned(),
        false => "Install".to_owned(),
    });
    let current = create_memo(clone!(run -> move || run.get().is_some_and(|run| run.current)));
    let cannot_install = create_memo(clone!(run can_install -> move || {
        !can_install.get() || !run.get().is_some_and(|run| run.succeeded && !run.installing)
    }));
    let error = create_memo(clone!(run -> move || run.get().and_then(|run| run.error)));
    let install = run.clone();
    view! {
        <Frame padding_vertical=4.0>
            <Frame
                color={theme.surface.clone()}
                outline={theme.border.clone()}
                outline_width=1.0
                outline_visible=true
                radius=6
                padding_horizontal=10.0
                padding_vertical=8.0
            >
                <List spacing=4.0>
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Heading content={title} />
                        <Show condition={current}>
                            <Caption content="running" color={theme.accent.clone()} />
                        </Show>
                    </List>
                    <Caption content={details} />
                    <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                        <Link
                            label="View on GitHub"
                            on_click={move || debug(DebugCommand::OpenUrl(url.get_untracked()))}
                        />
                        <Button
                            label={install_label}
                            variant=ButtonVariant::Secondary
                            disabled={cannot_install}
                            on_click={move || {
                                if let Some(run) = install.get_untracked() {
                                    debug(DebugCommand::Install(run.id));
                                }
                            }}
                        />
                    </List>
                    <ErrorText text={error} />
                </List>
            </Frame>
        </Frame>
    }
}

#[component]
fn TerminalWindow(terminal: Memo<Option<TerminalView>>) -> NodeId {
    let open = create_memo(clone!(terminal -> move || terminal.get().is_some()));
    let error =
        create_memo(clone!(terminal -> move || terminal.get().and_then(|terminal| terminal.error)));
    let failed = create_memo(clone!(error -> move || error.get().is_some()));
    view! {
        <Window
            open={open}
            title="Terminal"
            position={pos2(200.0, 120.0)}
            size={vec2(820.0, 520.0)}
            on_close={|| debug(DebugCommand::Close(DebugWindow::Terminal))}
        >
            <List spacing=8.0>
                <ErrorText text={error} />
                <Show condition={failed}>
                    <Button
                        label="Retry"
                        variant=ButtonVariant::Secondary
                        on_click={|| debug(DebugCommand::Terminal(TerminalInput::Retry))}
                    />
                </Show>
                <TerminalScreen @sizing=ItemSize::Percent(100.0) terminal />
            </List>
        </Window>
    }
}

#[component]
fn TerminalScreen(terminal: Memo<Option<TerminalView>>) -> NodeId {
    let size = component_size();
    create_effect(move || {
        let size = size.get();
        let Some(cell) = layout_text(
            "M",
            FontId::monospace(TERMINAL_FONT_SIZE),
            TextLayout::DEFAULT,
        )
        .map(|galley| galley.size()) else {
            return;
        };
        if cell.x <= 0.0 || cell.y <= 0.0 {
            return;
        }
        let cols = ((size.x - 2.0 * TERMINAL_PADDING) / cell.x)
            .floor()
            .max(1.0) as u16;
        let rows = ((size.y - 2.0 * TERMINAL_PADDING) / cell.y)
            .floor()
            .max(1.0) as u16;
        untrack(|| {
            debug(DebugCommand::Terminal(TerminalInput::Resize {
                cols,
                rows,
                cell_width: cell.x as u32,
                cell_height: cell.y as u32,
            }));
        });
    });
    let background = create_memo(clone!(terminal -> move || {
        terminal.get().map_or(Color32::BLACK, |terminal| terminal.background)
    }));
    let rows = create_memo(move || {
        terminal
            .get()
            .map(|terminal| terminal.rows)
            .unwrap_or_default()
    });
    let keys = create_memo(clone!(rows -> move || (0..rows.get().len()).collect::<Vec<_>>()));
    view! {
        <Focusable
            on_text={|text: String| debug(DebugCommand::Terminal(TerminalInput::Text(text)))}
            on_key={|press: KeyPress| {
                if press.pressed {
                    debug(DebugCommand::Terminal(TerminalInput::Key(press.key, press.modifiers)));
                }
                true
            }}
        >
            <ClickCatcher
                on_scroll={|gesture: ScrollGesture| {
                    let rows = (gesture.delta.y / (TERMINAL_FONT_SIZE * 1.2)).round() as isize;
                    if rows != 0 {
                        debug(DebugCommand::Terminal(TerminalInput::Scroll(-rows)));
                    }
                }}
            >
                <Frame
                    color={background}
                    padding_horizontal=TERMINAL_PADDING
                    padding_vertical=TERMINAL_PADDING
                >
                    <List spacing=0.0>
                        <ForEach keys={keys}>
                            {move |index: usize| {
                                let rows = rows.clone();
                                let row = create_memo(move || rows.get().get(index).cloned());
                                view! {
                                    <TerminalLine row />
                                }
                            }}
                        </ForEach>
                    </List>
                </Frame>
            </ClickCatcher>
        </Focusable>
    }
}

#[component]
fn TerminalLine(row: Memo<Option<TerminalRow>>) -> NodeId {
    let keys = create_memo(clone!(row -> move || {
        (0..row.get().map_or(0, |row| row.spans.len())).collect::<Vec<_>>()
    }));
    view! {
        <List direction=Direction::Horizontal spacing=0.0>
            <ForEach keys={keys}>
                {move |index: usize| {
                    let row = row.clone();
                    let span = create_memo(move || row.get().and_then(|row| row.spans.get(index).cloned()));
                    view! {
                        <TerminalCell span />
                    }
                }}
            </ForEach>
        </List>
    }
}

#[component]
fn TerminalCell(span: Memo<Option<TerminalSpan>>) -> NodeId {
    let field = |read: fn(&TerminalSpan) -> bool| {
        let span = span.clone();
        create_memo(move || span.get().is_some_and(|span| read(&span)))
    };
    let bold = field(|span| span.bold);
    let italic = field(|span| span.italic);
    let underline = field(|span| span.underline);
    let text =
        create_memo(clone!(span -> move || span.get().map(|span| span.text).unwrap_or_default()));
    let color =
        create_memo(clone!(span -> move || span.get().map_or(Color32::WHITE, |span| span.color)));
    let background = create_memo(move || {
        span.get()
            .and_then(|span| span.background)
            .unwrap_or(Color32::TRANSPARENT)
    });
    view! {
        <Frame color={background}>
            <Text
                string={text}
                font_size=TERMINAL_FONT_SIZE
                color={color}
                monospace=true
                bold
                italic
                underline
            />
        </Frame>
    }
}
