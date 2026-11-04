use super::organization::HostMenuPanel;
use be_host_base::performance::PerformanceRow;
use be_sdk_api::HostPanel;
use std::rc::Rc;

use beui::NodeId;
use beui::Overflow;
use beui::reactive::*;
use beui::styled::*;
use beui::{Color, LineJoin, Path, Rect, Stroke, pos2};

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

/// be-host's memory at one moment, or the peak of each through a minute, in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MemoryUsage {
    /// Resident in all: the three below summed.
    pub(crate) resident: u64,
    /// Heaps, wasm memories and stacks.
    pub(crate) anonymous: u64,
    /// Mapped files: the program, its libraries and fonts.
    pub(crate) file: u64,
    /// Shared memory: mostly the buffers of programs' windows, which count for them too.
    pub(crate) shared: u64,
    /// What malloc has taken from the system.
    pub(crate) heap: u64,
    /// Of that, what is free and not handed back.
    pub(crate) heap_free: u64,
    /// The linear memories of every running app and type module.
    pub(crate) wasm: u64,
}

#[cfg(target_os = "linux")]
impl MemoryUsage {
    pub(crate) fn peak(self, other: Self) -> Self {
        Self {
            resident: self.resident.max(other.resident),
            anonymous: self.anonymous.max(other.anonymous),
            file: self.file.max(other.file),
            shared: self.shared.max(other.shared),
            heap: self.heap.max(other.heap),
            heap_free: self.heap_free.max(other.heap_free),
            wasm: self.wasm.max(other.wasm),
        }
    }
}

/// A minute of memory statistics: when it began, in seconds since the Unix epoch, and the peak
/// of each measurement through it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MemoryPoint {
    pub(crate) at: u64,
    pub(crate) usage: MemoryUsage,
}

/// A measurement the memory graph draws, in the colour its row's name is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MemoryLine {
    Resident,
    Anonymous,
    Shared,
    Wasm,
}

impl MemoryLine {
    pub(crate) const ALL: [Self; 4] = [Self::Resident, Self::Anonymous, Self::Shared, Self::Wasm];

    fn of(self, usage: &MemoryUsage) -> u64 {
        match self {
            Self::Resident => usage.resident,
            Self::Anonymous => usage.anonymous,
            Self::Shared => usage.shared,
            Self::Wasm => usage.wasm,
        }
    }

    fn color(self) -> TextColor {
        match self {
            Self::Resident => TextColor::Accent,
            Self::Anonymous => TextColor::Success,
            Self::Shared => TextColor::Warning,
            Self::Wasm => TextColor::Danger,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MemoryRow {
    pub(crate) row: PerformanceRow,
    pub(crate) line: Option<MemoryLine>,
}

/// be-host's memory through this run: a point a minute, the minute so far last.
#[derive(Clone, Debug, Default)]
pub(crate) struct MemoryView {
    /// Counts the samples taken, so a view compares equal until the next one.
    pub(crate) generation: u64,
    pub(crate) points: std::sync::Arc<[MemoryPoint]>,
    /// When the latest sample was taken, in seconds since the Unix epoch.
    pub(crate) now: u64,
    pub(crate) rows: Vec<MemoryRow>,
    /// Where the statistics are written, if they are.
    pub(crate) file: Option<String>,
}

impl PartialEq for MemoryView {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DebugView {
    pub(crate) client: Option<Vec<Line>>,
    pub(crate) performance: Option<Vec<PerformanceRow>>,
    pub(crate) memory: Option<MemoryView>,
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
                            let memory = create_memo(move || debug.get().memory);
                            view! {
                                <ListItem grow=1.0>
                                    <PerformancePanel performance memory />
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
pub(crate) fn PerformancePanel(
    performance: Memo<Option<Vec<PerformanceRow>>>,
    memory: Memo<Option<MemoryView>>,
) -> NodeId {
    let rows = create_memo(move || performance.get().unwrap_or_default());
    let keys = create_memo(move || (0..rows.get().len()).collect::<Vec<_>>());
    let measured = create_memo(move || memory.get().is_some());
    view! {
        <Frame padding=PANEL_PADDING>
            <Scroll>
                <Show condition={measured}>
                    <MemoryGraph memory />
                </Show>
                <PerformanceLine
                    row={create_memo(|| Some(PerformanceRow {
                        heading: true,
                        name: "Measurement".to_owned(),
                        current: "Current".to_owned(),
                        average: "Average".to_owned(),
                        peak: "Peak".to_owned(),
                    }))}
                />
                <Show condition={measured}>
                    <MemoryRows memory />
                </Show>
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

const GRAPH_HEIGHT: f32 = 160.0;

#[component]
fn MemoryGraph(memory: Memo<Option<MemoryView>>) -> NodeId {
    let theme = use_theme();
    let draw: Prop<Draw> = Prop::dynamic(move || {
        let memory = memory.get().unwrap_or_default();
        let colors = MemoryLine::ALL.map(|line| match line.color() {
            TextColor::Accent => theme.accent.get(),
            TextColor::Success => theme.success.get(),
            TextColor::Warning => theme.warning.get(),
            _ => theme.danger.get(),
        });
        let border = theme.border.get();
        Rc::new(move |painter: &Painter, rect: Rect| {
            draw_memory(painter, rect, &memory, colors, border);
        }) as Draw
    });
    let caption = create_memo(move || {
        memory
            .get()
            .map(|memory| caption(&memory))
            .unwrap_or_default()
    });
    view! {
        <Frame padding={Padding::ZERO.bottom(Space::Lg)}>
            <List spacing=Space::Xs2>
                <Frame height={Size::Custom(GRAPH_HEIGHT)}>
                    <Drawing draw />
                </Frame>
                <Text style=TextStyle::CAPTION>{caption}</Text>
            </List>
        </Frame>
    }
}

#[component]
fn MemoryRows(memory: Memo<Option<MemoryView>>) -> NodeId {
    let rows = create_memo(move || memory.get().map(|memory| memory.rows).unwrap_or_default());
    let keys = create_memo(move || (0..rows.get().len()).collect::<Vec<_>>());
    view! {
        <Frame padding={Padding::ZERO.bottom(Space::Lg)}>
            <List spacing=Space::None>
                <ForEach keys={keys}>|index: usize| {
                    let row = create_memo(move || rows.get().get(index).cloned());
                    let color = create_memo(move || {
                        row.get()
                            .and_then(|row| row.line)
                            .map_or(TextColor::Default, MemoryLine::color)
                    });
                    view! {
                        <PerformanceLine
                            row={create_memo(move || row.get().map(|row| row.row))}
                            color
                        />
                    }
                }</ForEach>
            </List>
        </Frame>
    }
}

/// What the graph spans and what its top stands for.
fn caption(memory: &MemoryView) -> String {
    let span = memory
        .points
        .first()
        .map_or(0, |first| memory.now.saturating_sub(first.at));
    let (hours, minutes) = (span / 3600, (span / 60) % 60);
    let file = memory
        .file
        .as_ref()
        .map(|file| format!(" · written to {file}"))
        .unwrap_or_default();
    format!(
        "The last {hours}h {minutes:02}m, a point a minute at its peak; the top is {}{file}",
        crate::debug::bytes(graph_top(memory))
    )
}

/// The graph's top: the peak resident size rounded up to the next 64 MiB.
fn graph_top(memory: &MemoryView) -> u64 {
    const STEP: u64 = 64 << 20;
    let peak = memory
        .points
        .iter()
        .map(|point| point.usage.resident)
        .max()
        .unwrap_or_default();
    (peak / STEP + 1) * STEP
}

fn draw_memory(
    painter: &Painter,
    rect: Rect,
    memory: &MemoryView,
    colors: [Color; 4],
    border: Color,
) {
    painter.rect_stroke(rect, 0.0, 1.0, border);
    for quarter in 1..4 {
        let y = rect.top() + rect.height() * quarter as f32 / 4.0;
        painter.line(
            pos2(rect.left(), y),
            pos2(rect.right(), y),
            1.0,
            border.scale_alpha(0.5),
        );
    }
    let (Some(first), top) = (memory.points.first(), graph_top(memory)) else {
        return;
    };
    let span = memory.now.saturating_sub(first.at).max(60) as f32;
    let at = |point: &MemoryPoint, value: u64| {
        pos2(
            rect.left() + rect.width() * point.at.saturating_sub(first.at) as f32 / span,
            rect.bottom() - rect.height() * value as f32 / top as f32,
        )
    };
    for (line, color) in MemoryLine::ALL.into_iter().zip(colors).rev() {
        let mut path = Path::new();
        let mut previous: Option<u64> = None;
        for point in memory.points.iter() {
            let spot = at(point, line.of(&point.usage));
            path = match previous {
                // A gap of more than two minutes, while the computer slept, breaks the line.
                Some(before) if point.at.saturating_sub(before) <= 120 => path.line_to(spot),
                _ => path.move_to(spot),
            };
            previous = Some(point.at);
        }
        if let [only] = &*memory.points {
            let spot = at(only, line.of(&only.usage));
            path = path.line_to(pos2(rect.right(), spot.y));
        }
        painter.stroke_path(&path, &Stroke::new(1.5).join(LineJoin::Round), color);
    }
}

#[component]
fn PerformanceLine(
    row: Memo<Option<PerformanceRow>>,
    #[prop(default = TextColor::Default)] color: Prop<TextColor>,
) -> NodeId {
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
                <Text overflow=Overflow::Ellipsis size=TextSize::Sm weight={weight} color>
                    {name}
                </Text>
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
