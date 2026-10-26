use beui::NodeId;
use beui::Overflow;
use beui::reactive::*;
use beui::styled::*;
use block_plugin_api::HostPanel;

use super::{UiCommand, send};

#[derive(Clone, Debug)]
pub(crate) enum DebugCommand {
    KillPlugin(String),
    CrashPlugin(String),
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

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DebugView {
    pub(crate) client: Option<Vec<Line>>,
    pub(crate) performance: Option<Vec<PerformanceRow>>,
    pub(crate) plugins: Option<PluginsView>,
}

#[derive(Clone)]
pub(crate) struct DebugPanels(pub(crate) Memo<DebugView>);

#[component]
pub(crate) fn HostPanelSurface(panel: HostPanel) -> NodeId {
    crate::debug::panel_shown(panel);
    on_cleanup(move || crate::debug::panel_hidden(panel));
    let provided = use_context::<DebugPanels>();
    let debug = create_memo(move || {
        provided
            .as_ref()
            .map(|DebugPanels(debug)| debug.get())
            .unwrap_or_default()
    });
    let shown = create_memo(move || panel);
    view! {
        <Frame fill=ColorRole::Background>
            <List spacing=0.0>
                <Dynamic value={shown}>|panel: HostPanel| {
                    match panel {
                        HostPanel::BlockStack => {
                            let client = create_memo(move || debug.get().client);
                            view! {
                                <ClientPanel @sizing=ItemSize::Percent(100.0) client />
                            }
                        }
                        HostPanel::Performance => {
                            let performance = create_memo(move || debug.get().performance);
                            view! {
                                <PerformancePanel @sizing=ItemSize::Percent(100.0) performance />
                            }
                        }
                        HostPanel::Plugins => {
                            let plugins = create_memo(move || debug.get().plugins);
                            view! {
                                <PluginsPanel @sizing=ItemSize::Percent(100.0) plugins />
                            }
                        }
                    }
                }</Dynamic>
            </List>
        </Frame>
    }
}

fn debug(command: DebugCommand) {
    send(UiCommand::Debug(command));
}

const LINE_HEIGHT: f32 = 18.0;
const PANEL_PADDING: f32 = 12.0;
const INDENT: f32 = 14.0;

#[component]
fn LinesView(lines: Memo<Vec<Line>>) -> NodeId {
    let keys = create_memo(move || (0..lines.get().len()).collect::<Vec<_>>());
    view! {
        <Scroll>
            <VirtualList keys={keys} item_size=LINE_HEIGHT>|index: ReadSignal<usize>| {
                let line = create_memo(move || lines.with(|lines| lines.get(index.get()).cloned()));
                view! {
                    <LineView line />
                }
            }</VirtualList>
        </Scroll>
    }
}

#[component]
fn LineView(line: Memo<Option<Line>>) -> NodeId {
    let indent = create_memo(move || {
        Sides::new(
            line.get()
                .map_or(0.0, |line| f32::from(line.indent) * INDENT),
            0.0,
            0.0,
            0.0,
        )
    });
    let text = create_memo(move || line.get().map(|line| line.text).unwrap_or_default());
    let style = create_memo(move || line.get().map_or(LineStyle::Body, |line| line.style));
    let color = create_memo(move || match style.get() {
        LineStyle::Heading | LineStyle::Body | LineStyle::Code => TextColor::Default,
        LineStyle::Muted => TextColor::Muted,
        LineStyle::Error => TextColor::Danger,
    });
    let weight = create_memo(move || match style.get() {
        LineStyle::Heading => Weight::Bold,
        _ => Weight::Normal,
    });
    let family = create_memo(move || match style.get() {
        LineStyle::Code => Family::Mono,
        _ => Family::Sans,
    });
    let size = create_memo(move || match style.get() {
        LineStyle::Heading => TextSize::Base,
        _ => TextSize::Sm,
    });
    view! {
        <Frame padding=indent>
            <Text size color weight family>{text}</Text>
        </Frame>
    }
}

#[component]
fn ClientPanel(client: Memo<Option<Vec<Line>>>) -> NodeId {
    let lines = create_memo(move || client.get().unwrap_or_default());
    view! {
        <Frame padding=PANEL_PADDING>
            <LinesView lines />
        </Frame>
    }
}

#[component]
fn PerformancePanel(performance: Memo<Option<Vec<PerformanceRow>>>) -> NodeId {
    let rows = create_memo(move || performance.get().unwrap_or_default());
    let keys = create_memo(move || (0..rows.get().len()).collect::<Vec<_>>());
    view! {
        <Frame padding=PANEL_PADDING>
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
                <ForEach keys={keys}>|index: usize| {
                    let row = create_memo(move || rows.get().get(index).cloned());
                    view! {
                        <PerformanceLine row />
                    }
                }</ForEach>
            </Scroll>
        </Frame>
    }
}

#[component]
fn PerformanceLine(row: Memo<Option<PerformanceRow>>) -> NodeId {
    let cell = |read: fn(&PerformanceRow) -> String| {
        create_memo(move || row.get().map(|row| read(&row)).unwrap_or_default())
    };
    let name = cell(|row| row.name.clone());
    let current = cell(|row| row.current.clone());
    let average = cell(|row| row.average.clone());
    let peak = cell(|row| row.peak.clone());
    let weight = create_memo(move || match row.get().is_some_and(|row| row.heading) {
        true => Weight::Bold,
        false => Weight::Normal,
    });
    view! {
        <List direction=Direction::Horizontal spacing=8.0>
            <Text
                overflow=Overflow::Ellipsis
                @sizing=ItemSize::Percent(100.0)
                size=TextSize::Sm
                weight={weight}
            >
                {name}
            </Text>
            <Text
                @sizing=ItemSize::Fixed(90.0)
                size=TextSize::Sm
                family=Family::Mono
                weight={weight}
            >
                {current}
            </Text>
            <Text
                @sizing=ItemSize::Fixed(90.0)
                size=TextSize::Sm
                family=Family::Mono
                weight={weight}
            >
                {average}
            </Text>
            <Text
                @sizing=ItemSize::Fixed(90.0)
                size=TextSize::Sm
                family=Family::Mono
                weight={weight}
            >
                {peak}
            </Text>
        </List>
    }
}

#[component]
fn PluginsPanel(plugins: Memo<Option<PluginsView>>) -> NodeId {
    let lines = create_memo(move || {
        plugins
            .get()
            .map(|plugins| plugins.lines)
            .unwrap_or_default()
    });
    let line_keys = create_memo(move || (0..lines.get().len()).collect::<Vec<_>>());
    let runtimes = create_memo(move || {
        plugins
            .get()
            .map(|plugins| plugins.runtimes)
            .unwrap_or_default()
    });
    let runtime_keys = create_memo(move || {
        runtimes
            .get()
            .into_iter()
            .map(|runtime| runtime.id)
            .collect::<Vec<_>>()
    });
    let idle = create_memo(move || runtime_keys.get().is_empty());
    let running = create_memo(move || format!("Running ({})", runtime_keys.get().len()));
    view! {
        <Frame padding=PANEL_PADDING>
            <Scroll>
                <ForEach keys={line_keys}>|index: usize| {
                    let line = create_memo(move || lines.get().get(index).cloned());
                    view! {
                        <LineView line />
                    }
                }</ForEach>
                <Text style=TextStyle::HEADING>{running}</Text>
                <Show condition={idle}>
                    <Text style=TextStyle::CAPTION>"no editor plugins are running"</Text>
                </Show>
                <ForEach keys={runtime_keys}>|id: String| {
                    let key = id.clone();
                    let runtime = create_memo(move || {
                        runtimes.get().into_iter().find(|runtime| runtime.id == key)
                    });
                    view! {
                        <RuntimeBlock id runtime />
                    }
                }</ForEach>
            </Scroll>
        </Frame>
    }
}

#[component]
fn RuntimeBlock(id: String, runtime: Memo<Option<RuntimeView>>) -> NodeId {
    let state = create_memo(move || {
        runtime
            .get()
            .map(|runtime| runtime.state)
            .unwrap_or_default()
    });
    let lines = create_memo(move || {
        runtime
            .get()
            .map(|runtime| runtime.lines)
            .unwrap_or_default()
    });
    let keys = create_memo(move || (0..lines.get().len()).collect::<Vec<_>>());
    let kill = id.clone();
    let crash = id.clone();
    let crash_id = format!("debug.plugins.crash.{id}");
    view! {
        <Frame padding={Sides::vertical(6.0)}>
            <List spacing=2.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                    <Text style=TextStyle::HEADING>{id}</Text>
                    <Text style=TextStyle::CAPTION @sizing=ItemSize::Percent(100.0)>{state}</Text>
                    <Button
                        label="Kill"
                        variant=ButtonVariant::Secondary
                        on_click={move || debug(DebugCommand::KillPlugin(kill.clone()))}
                    />
                    <Button
                        label="Crash"
                        variant=ButtonVariant::Secondary
                        @test_id={crash_id}
                        on_click={move || debug(DebugCommand::CrashPlugin(crash.clone()))}
                    />
                </List>
                <ForEach keys={keys}>|index: usize| {
                    let line = create_memo(move || lines.get().get(index).cloned());
                    view! {
                        <LineView line />
                    }
                }</ForEach>
            </List>
        </Frame>
    }
}
