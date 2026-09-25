use beui::reactive::{
    Align, Direction, ForEach, Frame, ItemSize, List, Memo, Show, Spacer, Text, VirtualList, clone,
    component, create_memo, view,
};
use beui::styled::{
    Button, ButtonVariant, Caption, Code, Heading, Link, Scroll, Spinner, use_theme,
};
use beui::NodeId;

use super::onboarding::ErrorText;
use super::{UiCommand, send};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum DebugWindow {
    Client,
    Performance,
    Plugins,
    Version,
}

#[derive(Clone, Debug)]
pub(crate) enum DebugCommand {
    Open(DebugWindow),
    Close(DebugWindow),
    KillPlugin(String),
    RefreshVersions,
    Install(u64),
    OpenUrl(String),
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

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DebugView {
    pub(crate) client: Option<Vec<Line>>,
    pub(crate) performance: Option<Vec<PerformanceRow>>,
    pub(crate) plugins: Option<PluginsView>,
    pub(crate) version: Option<VersionView>,
}

fn debug(command: DebugCommand) {
    send(UiCommand::Debug(command));
}

const LINE_HEIGHT: f32 = 18.0;
const PANEL_PADDING: f32 = 12.0;
const INDENT: f32 = 14.0;

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
pub(super) fn ClientPanel(client: Memo<Option<Vec<Line>>>) -> NodeId {
    let lines = create_memo(move || client.get().unwrap_or_default());
    view! {
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
            <LinesView lines />
        </Frame>
    }
}

#[component]
pub(super) fn PerformancePanel(performance: Memo<Option<Vec<PerformanceRow>>>) -> NodeId {
    let rows = create_memo(move || performance.get().unwrap_or_default());
    let keys = create_memo(clone!(rows -> move || (0..rows.get().len()).collect::<Vec<_>>()));
    view! {
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
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
        </Frame>
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
pub(super) fn PluginsPanel(plugins: Memo<Option<PluginsView>>) -> NodeId {
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
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
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
        </Frame>
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
pub(super) fn VersionPanel(version: Memo<Option<VersionView>>) -> NodeId {
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
        <Frame padding_horizontal=PANEL_PADDING padding_vertical=PANEL_PADDING>
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
        </Frame>
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
