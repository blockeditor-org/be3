use super::organization::HostMenuPanel;
use be_host_base::performance::PerformanceRow;
use be_sdk_api::HostPanel;
use beui::NodeId;
use beui::Overflow;
use beui::reactive::*;
use beui::styled::*;

use super::{UiCommand, send};
use beui::{Align, Direction};

#[derive(Clone, Debug)]
pub(crate) enum DebugCommand {
    KillApp(String),
    CrashApp(String),
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
pub(crate) struct RuntimeView {
    pub(crate) id: String,
    pub(crate) state: String,
    pub(crate) lines: Vec<Line>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct AppsView {
    pub(crate) lines: Vec<Line>,
    pub(crate) runtimes: Vec<RuntimeView>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DebugView {
    pub(crate) client: Option<Vec<Line>>,
    pub(crate) performance: Option<Vec<PerformanceRow>>,
    pub(crate) apps: Option<AppsView>,
}

#[derive(Clone)]
pub(crate) struct DebugPanels(pub(crate) Memo<DebugView>);

#[component]
pub(crate) fn HostPanelSurface(panel: HostPanel) -> NodeId {
    if panel == HostPanel::Menu {
        return view! {
            <HostMenuPanel />
        };
    }
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
            <List spacing=Space::None>
                <Dynamic value={shown}>|panel: HostPanel| {
                    match panel {
                        HostPanel::BlockStack => {
                            let client = create_memo(move || debug.get().client);
                            view! {
                                <ListItem grow=1.0>
                                    <ClientPanel client />
                                </ListItem>
                            }
                        }
                        HostPanel::Performance => {
                            let performance = create_memo(move || debug.get().performance);
                            view! {
                                <ListItem grow=1.0>
                                    <PerformancePanel performance />
                                </ListItem>
                            }
                        }
                        HostPanel::Apps => {
                            let apps = create_memo(move || debug.get().apps);
                            view! {
                                <ListItem grow=1.0>
                                    <AppsPanel apps />
                                </ListItem>
                            }
                        }
                        HostPanel::Menu => view! {
                            <Spacer />
                        },
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
const PANEL_PADDING: Space = Space::Lg;
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
        Padding::ZERO.left(Space::Custom(
            line.get()
                .map_or(0.0, |line| f32::from(line.indent) * INDENT),
        ))
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
        <List direction=Direction::Horizontal spacing=Space::Md>
            <ListItem grow=1.0>
                <Text overflow=Overflow::Ellipsis size=TextSize::Sm weight={weight}>{name}</Text>
            </ListItem>
            <ListItem length=90.0>
                <Text size=TextSize::Sm family=Family::Mono weight={weight}>{current}</Text>
            </ListItem>
            <ListItem length=90.0>
                <Text size=TextSize::Sm family=Family::Mono weight={weight}>{average}</Text>
            </ListItem>
            <ListItem length=90.0>
                <Text size=TextSize::Sm family=Family::Mono weight={weight}>{peak}</Text>
            </ListItem>
        </List>
    }
}

#[component]
fn AppsPanel(apps: Memo<Option<AppsView>>) -> NodeId {
    let lines = create_memo(move || apps.get().map(|apps| apps.lines).unwrap_or_default());
    let line_keys = create_memo(move || (0..lines.get().len()).collect::<Vec<_>>());
    let runtimes = create_memo(move || apps.get().map(|apps| apps.runtimes).unwrap_or_default());
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
                    <Text style=TextStyle::CAPTION>"no apps are running"</Text>
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
    let kill_id = format!("debug.apps.kill.{id}");
    let crash_id = format!("debug.apps.crash.{id}");
    view! {
        <Frame padding={Padding::vertical(Space::Sm)}>
            <List spacing=Space::Xs2>
                <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                    <Text size=TextSize::Lg>{id}</Text>
                    <ListItem grow=1.0>
                        <Text style=TextStyle::CAPTION>{state}</Text>
                    </ListItem>
                    <Button
                        label="Kill"
                        variant=ButtonVariant::Secondary
                        @test_id={kill_id}
                        on_click={move || debug(DebugCommand::KillApp(kill.clone()))}
                    />
                    <Button
                        label="Crash"
                        variant=ButtonVariant::Secondary
                        @test_id={crash_id}
                        on_click={move || debug(DebugCommand::CrashApp(crash.clone()))}
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
