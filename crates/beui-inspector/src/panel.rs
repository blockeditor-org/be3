use beui_core::font::Overflow;
use stable_hash::HashMap;
use std::time::Duration;

use crate::screen_reader::Command;
use beui_components_styled::*;
use beui_components_unstyled::{ChoiceOption, SliderScale, TreeItem};
use beui_core::base::TextAlign;
use beui_core::base::embed::EmbedSlot;
use beui_core::current::{enter, with_document};
use beui_core::document::Document;
use beui_core::filter::{ColorVision, MAX_BLUR};
use beui_core::icons::{ICON_MOUSE_POINTER_2, ICON_X};
use beui_core::input::{BackEdge, BackGesture};
use beui_core::node::NodeId;
use beui_core::performance::PerformanceSnapshot;
use beui_core::performance::PerformanceTimings;
use beui_core::preferences::{Motion, PreferenceLayer, ThemeKind};
use beui_dock::styled::Docking;
use beui_dock::unstyled::{DockPane, DockSplit, DockTab, DockingLayout, dock_menu, use_dock_tab};
use beui_view::reactive::*;

use super::tree::{Entry, Key};
use super::{DEFAULT_WIDTH, InspectorTab, State, entry_label};
use beui_view::types::{Align, Direction, ItemSize};

const HEADER_SPACING: Space = Space::Md;
const BODY_PADDING: Space = Space::Md;
const FOOTER_PADDING: Space = Space::Md;
const FOOTER_SPACING: Space = Space::Xs2;
const ROW_SPACING: Space = Space::Sm;
const PERFORMANCE_SPACING: Space = Space::Md;
const TIMING_SPACING: Space = Space::Xs;
const BLUR_MIDPOINT: f32 = 12.0;
const RENDERER_LABEL_WIDTH: f32 = 104.0;
const PROPERTIES_SHARE: f32 = 45.0;
pub const PICK: &str = "inspector.pick";
pub const CLOSE: &str = "inspector.close";
const PIXEL_RATIOS: [(&str, Option<f32>); 5] = [
    ("Native", None),
    ("1x", Some(1.0)),
    ("1.5x", Some(1.5)),
    ("2x", Some(2.0)),
    ("3x", Some(3.0)),
];

fn labels<T>(options: &[(&'static str, T)]) -> Vec<&'static str> {
    options.iter().map(|(label, _)| *label).collect()
}

const NO_OVERRIDE: &str = "No override";

fn override_index<T: PartialEq>(choices: &[T], chosen: Option<T>) -> usize {
    chosen
        .and_then(|chosen| choices.iter().position(|choice| *choice == chosen))
        .map_or(0, |index| index + 1)
}

fn overridden<T: Copy>(choices: &[T], index: Option<usize>) -> Option<T> {
    index
        .and_then(|index| index.checked_sub(1))
        .and_then(|index| choices.get(index).copied())
}
const COMMANDS: [(&str, &str, Command); 12] = [
    ("First", "first", Command::First),
    ("Last", "last", Command::Last),
    ("Previous", "previous", Command::Previous),
    ("Next", "next", Command::Next),
    ("Prev control", "previous_control", Command::PreviousControl),
    ("Next control", "next_control", Command::NextControl),
    ("Activate", "activate", Command::Activate),
    ("Repeat", "repeat", Command::Repeat),
    ("Value down", "decrement", Command::Decrement),
    ("Value up", "increment", Command::Increment),
    ("Scroll up", "scroll_up", Command::ScrollUp),
    ("Scroll down", "scroll_down", Command::ScrollDown),
];
const KEYBOARD_GUIDE: [&str; 8] = [
    "Alt+Arrows - previous or next item",
    "Alt+Shift+Arrows - previous or next control",
    "Alt+Enter or Alt+Space - activate",
    "Alt+Home and Alt+End - first or last item",
    "Alt+Minus and Alt+Plus - adjust the value",
    "Alt+Page Up and Alt+Page Down - scroll",
    "Alt+R - repeat",
    "Other keys - the focused control",
];
const TOUCH_GUIDE: [&str; 5] = [
    "Drag a finger - read what is under it",
    "Flick sideways - previous or next item",
    "Flick up or down - adjust the value",
    "Double tap - activate",
    "Two finger tap - repeat, drag two fingers - scroll",
];

#[derive(Clone, Default, PartialEq)]
pub struct Summary {
    pub native_pixel_ratio: String,
    pub picking: bool,
    pub responsive: bool,
    pub handles_back: bool,
    pub bounds: String,
    pub properties: Vec<(String, String)>,
}

#[derive(Clone, Default, PartialEq)]
pub struct PerformanceSummary {
    samples: String,
    current: PerformanceTimings,
    average: PerformanceTimings,
    peak: PerformanceTimings,
    latest_work: String,
    scene: String,
    cache: String,
    reuse: String,
}

impl From<PerformanceSnapshot> for PerformanceSummary {
    fn from(snapshot: PerformanceSnapshot) -> Self {
        let samples = match snapshot.samples {
            1 => "1 sample".to_owned(),
            count => format!("{count} samples"),
        };
        let latest = snapshot.latest;
        let (latest_work, scene, cache, reuse) = if snapshot.samples == 0 {
            (
                "Waiting for a document update".to_owned(),
                String::new(),
                "Cache: waiting for measurements".to_owned(),
                String::new(),
            )
        } else {
            let work = latest.work;
            (
                format!(
                    "Latest: {} layout passes | paint {}",
                    latest.layout_passes,
                    if latest.painted { "ran" } else { "cached" }
                ),
                format!("Scene: {} nodes | {} shapes", latest.nodes, latest.shapes),
                format!(
                    "Cache: layout {}% | paint {}%",
                    percentage(snapshot.layout_cache_hits, snapshot.samples),
                    percentage(snapshot.paint_cache_hits, snapshot.samples)
                ),
                format!(
                    "Reused: {} of {} measured | {} of {} painted | {} accessibility nodes rebuilt of {} walked",
                    work.reused_measurements,
                    work.reused_measurements + work.measured,
                    work.replayed_nodes,
                    work.replayed_nodes + work.painted_nodes,
                    work.described_nodes,
                    work.walked_nodes
                ),
            )
        };
        Self {
            samples,
            current: latest.timings,
            average: snapshot.average,
            peak: snapshot.peak,
            latest_work,
            scene,
            cache,
            reuse,
        }
    }
}

type Entries = Signal<HashMap<Key, Entry>>;
pub type RendererRows = Vec<(&'static str, String)>;

#[derive(Clone, Copy)]
pub struct TreeView {
    pub keys: Signal<Vec<Key>>,
    pub entries: Entries,
    pub selection: Signal<Vec<Key>>,
    pub selection_text: Signal<String>,
    pub total: Signal<usize>,
    pub tree: ComponentRef<beui_components_styled::TreeApi<Key>>,
}

impl TreeView {
    fn new() -> Self {
        Self {
            keys: create_signal(Vec::new()),
            entries: create_signal(HashMap::default()),
            selection: create_signal(Vec::new()),
            selection_text: create_signal(String::new()),
            total: create_signal(0),
            tree: ComponentRef::new(),
        }
    }
}

pub struct Panel {
    pub document: Document,
    pub scope: Scope,
    pub trees: [TreeView; 3],
    pub tools: DockingLayout<InspectorTab>,
    pub summary: Signal<Summary>,
    pub performance: Signal<PerformanceSummary>,
    pub renderer: Signal<RendererRows>,
    pub app: EmbedSlot,
    pub width: Signal<f32>,
}

pub fn build(state: Stored<State>, width: f32, embeds_app: bool) -> Panel {
    let owner = RootScope::new();
    let trees = [TreeView::new(), TreeView::new(), TreeView::new()];
    let tools = DockingLayout::new();
    let summary = create_signal(Summary::default());
    let performance = create_signal(PerformanceSummary::default());
    let renderer = create_signal(RendererRows::new());
    let width = create_signal(width);
    let fraction = create_memo(move || app_share(width.get()));
    let app = EmbedSlot::new();

    let mut document = Document::new();
    let root = enter(&mut document, || {
        provide_context(ThemeStore::new(ThemeTokens::literal(&Theme::DARK)));
        let [beui, components, accesskit] = trees;
        let tool_pane = view! {
            <DockPane id="tools">
                <DockTab id=InspectorTab::Beui title={InspectorTab::Beui.title()}>
                    <TreePanel view=beui noun="node" state summary />
                </DockTab>
                <DockTab id=InspectorTab::Components title={InspectorTab::Components.title()}>
                    <TreePanel view=components noun="component" state summary />
                </DockTab>
                <DockTab id=InspectorTab::AccessKit title={InspectorTab::AccessKit.title()}>
                    <TreePanel view=accesskit noun="node" state summary />
                </DockTab>
                <DockTab id=InspectorTab::Performance title={InspectorTab::Performance.title()}>
                    <PerformanceTool
                        performance={performance.read_only()}
                        renderer={renderer.read_only()}
                        state
                    />
                </DockTab>
                <DockTab id=InspectorTab::Simulation title={InspectorTab::Simulation.title()}>
                    <SimulationTool state summary />
                </DockTab>
            </DockPane>
        };
        let layout = match embeds_app {
            true => view! {
                <DockSplit id="inspector" direction=Direction::Horizontal fraction={fraction}>
                    <DockPane id="app">
                        <DockTab
                            id=InspectorTab::App
                            title={InspectorTab::App.title()}
                            see_through=true
                        >
                            <AppTool app state />
                        </DockTab>
                    </DockPane>
                    {tool_pane}
                </DockSplit>
            },
            false => tool_pane,
        };
        view! {
            <Frame @test_id={"inspector.dock"}>
                <Docking layout=tools focus={Some(InspectorTab::Beui)}>{layout}</Docking>
            </Frame>
        }
    });
    document.set_root(root);

    Panel {
        document,
        scope: owner.into_scope(),
        trees,
        tools,
        summary,
        performance,
        renderer,
        app,
        width,
    }
}

fn app_share(width: f32) -> f32 {
    match width > DEFAULT_WIDTH {
        true => 1.0 - DEFAULT_WIDTH / width,
        false => 0.5,
    }
}

#[component]
fn AppTool(app: EmbedSlot, state: Stored<State>) -> NodeId {
    if let Some(tab) = use_dock_tab() {
        create_effect(move || {
            if tab.keyboard().is_some() {
                untrack(|| state.value().release_keyboard());
            }
        });
    }
    let embed = with_document(|document| document.create_embed(app.0.value())).id();
    app.0.with(|held| held.node.set(Some(embed)));
    test_id(embed, "inspector.app")
}

fn close_action(state: Stored<State>) -> Action {
    Action::new(CLOSE, "Close inspector", move || state.value().close())
        .glyph(ICON_X)
        .pin(Pin::Icon)
        .detached()
}

#[component]
fn TreePanel(
    view: TreeView,
    noun: &'static str,
    state: Stored<State>,
    summary: Signal<Summary>,
) -> NodeId {
    let TreeView {
        keys,
        entries,
        selection,
        selection_text,
        total,
        tree,
    } = view;
    let count_text = create_memo(move || total_label(total.get(), noun));
    let bounds_text = create_memo(move || summary.with(|summary| summary.bounds.clone()));
    let properties = create_memo(move || summary.with(|summary| summary.properties.clone()));
    let has_properties = create_memo(move || properties.with(|properties| !properties.is_empty()));
    let selected = create_memo(move || selection.with(|path| path.last().copied()));
    let picking = create_memo(move || summary.with(|summary| summary.picking));
    let pick = Action::new(PICK, "Pick", move || state.value().toggle_picking())
        .glyph(ICON_MOUSE_POINTER_2)
        .checked(picking)
        .pin(Pin::Icon)
        .detached();
    let close = close_action(state);
    dock_menu(create_memo(move || {
        vec![MenuNode::action(&pick), MenuNode::action(&close)]
    }));
    view! {
        <List spacing=Space::None>
            <List @sizing=ItemSize::Percent(100.0) spacing=Space::None>
                <Tree
                    @sizing=ItemSize::Percent(100.0 - PROPERTIES_SHARE)
                    api=tree
                    keys
                    item={move |key: Key| item(&entries, key)}
                    selected={selected}
                    ancestors={move |key: Key| ancestors(selection.read_only(), key)}
                    padding=BODY_PADDING
                    row_test_id={move |key: Key| key.test_id()}
                    reveal_test_id={"inspector.reveal".to_owned()}
                    on_select={move |key: Key| state.value().select_row(key)}
                    on_expand={move |(key, expanded): (Key, bool)| {
                        state.value().set_expanded(key, expanded);
                    }}
                    on_reveal={move |key: Key| {
                        state.value().expand_ancestors(&ancestors(selection.read_only(), key));
                    }}
                    on_hover_change={move |(key, hovered): (Key, bool)| {
                        state.value().hover(key.node(), hovered);
                    }}
                >|row: TreeRowFace<Key>|
                    <TreeCells row_key={row.key} entries={entries} />
                </Tree>
                <Show condition={has_properties}>
                    <Frame
                        @sizing={ItemSize::Percent(PROPERTIES_SHARE)}
                        @test_id={"inspector.properties"}
                    >
                        <PropertiesSection properties />
                    </Frame>
                </Show>
            </List>
            <Separator />
            <Frame padding=FOOTER_PADDING>
                <List spacing=FOOTER_SPACING>
                    <Text style=TextStyle::CODE>{selection_text}</Text>
                    <List
                        direction=Direction::Horizontal
                        align=Align::Center
                        spacing=HEADER_SPACING
                    >
                        <Text
                            style=TextStyle::CODE
                            color=TextColor::Muted
                            @sizing=ItemSize::Percent(100.0)
                        >
                            {bounds_text}
                        </Text>
                        <Text style=TextStyle::CAPTION>{count_text}</Text>
                    </List>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn PerformanceTool(
    performance: ReadSignal<PerformanceSummary>,
    renderer: ReadSignal<RendererRows>,
    state: Stored<State>,
) -> NodeId {
    let samples = create_memo(move || performance.with(|performance| performance.samples.clone()));
    let close = close_action(state);
    dock_menu(create_memo(move || vec![MenuNode::action(&close)]));
    view! {
        <List spacing=Space::None>
            <PerformancePanel
                @sizing=ItemSize::Percent(100.0)
                @test_id={"inspector.performance"}
                performance
                renderer
                state
            />
            <Separator />
            <Frame padding=FOOTER_PADDING>
                <List direction=Direction::Horizontal align=Align::Center spacing=HEADER_SPACING>
                    <Button
                        label="Reset samples"
                        variant=ButtonVariant::Secondary
                        on_click={move || state.value().reset_performance()}
                    />
                    <Text
                        style=TextStyle::CAPTION
                        @sizing=ItemSize::Percent(100.0)
                        align=TextAlign::End
                    >
                        {samples}
                    </Text>
                </List>
            </Frame>
        </List>
    }
}

#[component]
fn SimulationTool(state: Stored<State>, summary: Signal<Summary>) -> NodeId {
    let native_pixel_ratio_text =
        create_memo(move || summary.with(|summary| summary.native_pixel_ratio.clone()));
    let responsive = create_memo(move || summary.with(|summary| summary.responsive));
    let handles_back = create_memo(move || summary.with(|summary| summary.handles_back));
    let close = close_action(state);
    dock_menu(create_memo(move || vec![MenuNode::action(&close)]));
    view! {
        <List spacing=Space::None>
            <SimulationPanel @sizing=ItemSize::Percent(100.0) state responsive handles_back />
            <Separator />
            <Frame padding=FOOTER_PADDING>
                <Text style=TextStyle::CODE color=TextColor::Muted>{native_pixel_ratio_text}</Text>
            </Frame>
        </List>
    }
}

#[component]
fn SimulationPanel(
    state: Stored<State>,
    responsive: Memo<bool>,
    handles_back: Memo<bool>,
) -> NodeId {
    let simulated = state.value().simulated_pixels_per_point.get();
    let selected = PIXEL_RATIOS
        .iter()
        .position(|(_, ratio)| *ratio == simulated)
        .unwrap_or(0);
    let ratio = create_signal(Some(selected));
    let developer = state.value().developer.get();
    let theme = create_signal(Some(override_index(&ThemeKind::ALL, developer.theme)));
    let motion = create_signal(Some(override_index(&Motion::ALL, developer.motion)));
    view! {
        <Scroll>
            <Frame padding=BODY_PADDING>
                <List spacing=PERFORMANCE_SPACING>
                    <Checkbox
                        @test_id={"inspector.simulation.touch_emulation"}
                        label="Emulate touch with mouse"
                        checked={Binding::new(state.value().touch_emulation.get(), move |enabled| state.value().touch_emulation.set(enabled))}
                    />
                    <Checkbox
                        @test_id={"inspector.simulation.mouse_simulation"}
                        label="Simulate mouse with touch"
                        checked={Binding::new(state.value().mouse_simulation.get(), move |enabled| state.value().mouse_simulation.set(enabled))}
                    />
                    <Button
                        @test_id={"inspector.simulation.keyboard"}
                        label="Show or hide the on-screen keyboard"
                        variant=ButtonVariant::Secondary
                        on_click={move || state.value().toggle_keyboard()}
                    />
                    <Separator />
                    <List spacing=TIMING_SPACING>
                        <Text style=TextStyle::HEADING>"Device pixel ratio"</Text>
                        <Select
                            @test_id={"inspector.simulation.pixel_ratio"}
                            label="Device pixel ratio"
                            options={view! {
                                <ForEach keys={labels(&PIXEL_RATIOS)}>|label: &'static str|
                                    <ChoiceOption label />
                                </ForEach>
                            }}
                            selected={Binding::new(ratio, move |index: Option<usize>| {
                                ratio.set(index);
                                let ratio = index.and_then(|index| PIXEL_RATIOS.get(index));
                                state.value().simulate_pixels_per_point(ratio.and_then(|(_, ratio)| *ratio));
                            })}
                        />
                    </List>
                    <Separator />
                    <Checkbox
                        @test_id={"inspector.simulation.responsive"}
                        label="Responsive design mode"
                        checked={Binding::new(responsive, move |_| state.value().toggle_responsive())}
                    />
                    <Separator />
                    <List spacing=TIMING_SPACING>
                        <Text style=TextStyle::HEADING>"Theme"</Text>
                        <Select
                            @test_id={"inspector.simulation.theme"}
                            label="Theme"
                            options={view! {
                                <ChoiceOption label=NO_OVERRIDE />
                                <ForEach keys={ThemeKind::ALL.to_vec()}>|kind: ThemeKind|
                                    <ChoiceOption label={kind.name()} />
                                </ForEach>
                            }}
                            selected={Binding::new(theme, move |index: Option<usize>| {
                                theme.set(index);
                                let chosen = overridden(&ThemeKind::ALL, index);
                                let developer = state.value().developer.get();
                                state.value().override_preferences(PreferenceLayer { theme: chosen, ..developer });
                            })}
                        />
                    </List>
                    <List spacing=TIMING_SPACING>
                        <Text style=TextStyle::HEADING>"Motion"</Text>
                        <Select
                            @test_id={"inspector.simulation.motion"}
                            label="Motion"
                            options={view! {
                                <ChoiceOption label=NO_OVERRIDE />
                                <ForEach keys={Motion::ALL.to_vec()}>|motion: Motion|
                                    <ChoiceOption label={motion.name()} />
                                </ForEach>
                            }}
                            selected={Binding::new(motion, move |index: Option<usize>| {
                                motion.set(index);
                                let chosen = overridden(&Motion::ALL, index);
                                let developer = state.value().developer.get();
                                state.value().override_preferences(PreferenceLayer { motion: chosen, ..developer });
                            })}
                        />
                    </List>
                    <Separator />
                    <BackSection state={state} handles_back />
                    <Separator />
                    <FilterSection state={state} />
                    <Separator />
                    <ScreenReaderSection state={state} />
                </List>
            </Frame>
        </Scroll>
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Swipe {
    Idle,
    Swiping,
    Done,
}

#[component]
fn BackSection(state: Stored<State>, handles_back: Memo<bool>) -> NodeId {
    let progress = create_signal(0.0_f32);
    let swipe = StoredCell::new(Swipe::Idle);
    let dragging = StoredCell::new(false);
    let from_right = create_signal(false);
    let caption = create_memo(move || match handles_back.get() {
        true => "Drag to the end to go back, or let go early to cancel.".to_owned(),
        false => "Nothing in the app takes back: the system would leave it.".to_owned(),
    });
    let moved = move |value: f32| {
        progress.set(value);
        match swipe.get() {
            Swipe::Done => return,
            Swipe::Idle if value <= 0.0 => return,
            Swipe::Idle => {
                swipe.set(Swipe::Swiping);
                state.value().simulate_back(BackGesture::Started {
                    edge: match from_right.get_untracked() {
                        true => BackEdge::Right,
                        false => BackEdge::Left,
                    },
                });
            }
            Swipe::Swiping => {}
        }
        state.value().simulate_back(BackGesture::Progressed(value));
        if value >= 1.0 {
            state.value().simulate_back(BackGesture::Invoked);
            progress.set(0.0);
            swipe.set(match dragging.get() {
                true => Swipe::Done,
                false => Swipe::Idle,
            });
        } else if value <= 0.0 && !dragging.get() {
            state.value().simulate_back(BackGesture::Cancelled);
            swipe.set(Swipe::Idle);
        }
    };
    let released = move |held: bool| {
        dragging.set(held);
        if held {
            return;
        }
        if swipe.replace(Swipe::Idle) == Swipe::Swiping {
            state.value().simulate_back(BackGesture::Cancelled);
        }
        progress.set(0.0);
    };
    view! {
        <List spacing=TIMING_SPACING>
            <Text style=TextStyle::HEADING>"Back gesture"</Text>
            <Text style=TextStyle::CAPTION>{caption}</Text>
            <Slider
                @test_id={"inspector.simulation.back"}
                label="Back gesture progress"
                value={Binding::new(progress, moved)}
                on_drag_change={released}
            />
            <Checkbox
                @test_id={"inspector.simulation.back_from_right"}
                label="Swipe from the right edge"
                checked={from_right}
            />
        </List>
    }
}

#[component]
fn FilterSection(state: Stored<State>) -> NodeId {
    let blur = create_signal(state.value().blur.get());
    let contrast = create_signal(state.value().contrast_reduction.get());
    let blur_text = create_memo(move || blur_label(blur.get()));
    let contrast_text = create_memo(move || contrast_label(contrast.get()));
    let vision = state.value().color_vision.get();
    let selected_vision = ColorVision::ALL
        .iter()
        .position(|(_, candidate)| *candidate == vision)
        .unwrap_or(0);
    let vision = create_signal(Some(selected_vision));
    let vision_labels = labels(&ColorVision::ALL);
    view! {
        <List spacing=TIMING_SPACING>
            <Text style=TextStyle::HEADING>"Filters"</Text>
            <Text style=TextStyle::CAPTION>{blur_text}</Text>
            <Slider
                @test_id={"inspector.simulation.blur"}
                label="Blur"
                value={Binding::new(blur, move |value| {
                    blur.set(value);
                    state.value().set_blur(value);
                })}
                min=0.0
                max=MAX_BLUR
                scale={SliderScale::Midpoint(BLUR_MIDPOINT)}
            />
            <Text style=TextStyle::CAPTION>{contrast_text}</Text>
            <Slider
                @test_id={"inspector.simulation.contrast"}
                label="Reduce contrast"
                value={Binding::new(contrast, move |value| {
                    contrast.set(value);
                    state.value().set_contrast_reduction(value);
                })}
            />
            <Text style=TextStyle::CAPTION>"Colour vision"</Text>
            <Select
                @test_id={"inspector.simulation.color_vision"}
                label="Colour vision"
                options={view! {
                    <ForEach keys={vision_labels}>|label: &'static str|
                        <ChoiceOption label />
                    </ForEach>
                }}
                selected={Binding::new(vision, move |index: Option<usize>| {
                    vision.set(index);
                    if let Some((_, vision)) = index.and_then(|index| ColorVision::ALL.get(index)) {
                        state.value().choose_color_vision(*vision);
                    }
                })}
            />
        </List>
    }
}

fn blur_label(radius: f32) -> String {
    if radius < BLUR_MIDPOINT {
        format!("Blur: {radius:.1} px")
    } else {
        format!("Blur: {} px", radius.round())
    }
}

fn contrast_label(amount: f32) -> String {
    format!("Reduce contrast: {}%", (amount * 100.0).round())
}

#[component]
fn ScreenReaderSection(state: Stored<State>) -> NodeId {
    view! {
        <List spacing=TIMING_SPACING>
            <Text style=TextStyle::HEADING>"Screen reader"</Text>
            <Checkbox
                @test_id={"inspector.accessibility.enabled"}
                label="Expose the accessibility tree"
                checked={Binding::new(state.value().accessibility.get(), move |enabled| state.value().enable_accessibility(enabled))}
            />
            <Checkbox
                @test_id={"inspector.screen_reader.enabled"}
                label="Simulate a screen reader"
                checked={Binding::new(state.value().screen_reader.get(), move |enabled| state.value().enable_screen_reader(enabled))}
            />
            <CommandRow row=0 state />
            <CommandRow row=1 state={state} />
            <CommandRow row=2 state={state} />
            <CommandRow row=3 state={state} />
            <CommandRow row=4 state={state} />
            <CommandRow row=5 state={state} />
            <GuideSection title="Keyboard" lines={KEYBOARD_GUIDE.as_slice()} />
            <GuideSection title="Touch" lines={TOUCH_GUIDE.as_slice()} />
        </List>
    }
}

#[component]
fn GuideSection(title: &'static str, lines: &'static [&'static str]) -> NodeId {
    view! {
        <List spacing=FOOTER_SPACING>
            <Text style=TextStyle::CAPTION color=TextColor::Default>{title.to_owned()}</Text>
            <ForEach keys={lines.to_vec()}>|line: &'static str|
                <Text style=TextStyle::CAPTION>{line.to_owned()}</Text>
            </ForEach>
        </List>
    }
}

#[component]
fn CommandRow(row: usize, state: Stored<State>) -> NodeId {
    let (left_label, left_id, left) = COMMANDS[row * 2];
    let (right_label, right_id, right) = COMMANDS[row * 2 + 1];
    view! {
        <List direction=Direction::Horizontal spacing=TIMING_SPACING>
            <Button
                @sizing=ItemSize::Percent(100.0)
                @test_id={format!("inspector.screen_reader.{left_id}")}
                label={left_label.to_owned()}
                variant=ButtonVariant::Secondary
                on_click={move || state.value().command(left)}
            />
            <Button
                @sizing=ItemSize::Percent(100.0)
                @test_id={format!("inspector.screen_reader.{right_id}")}
                label={right_label.to_owned()}
                variant=ButtonVariant::Secondary
                on_click={move || state.value().command(right)}
            />
        </List>
    }
}

pub fn total_label(total: usize, noun: &str) -> String {
    match total {
        1 => format!("1 {noun}"),
        total => format!("{total} {noun}s"),
    }
}

#[component]
fn PerformancePanel(
    performance: ReadSignal<PerformanceSummary>,
    renderer: ReadSignal<RendererRows>,
    state: Stored<State>,
) -> NodeId {
    let latest_work = performance_text(performance, |summary| &summary.latest_work);
    let scene = performance_text(performance, |summary| &summary.scene);
    let cache = performance_text(performance, |summary| &summary.cache);
    let reuse = performance_text(performance, |summary| &summary.reuse);
    let total = timing_values(performance, |timings| timings.total);
    let layout = timing_values(performance, |timings| timings.layout);
    let interaction = timing_values(performance, |timings| timings.interaction);
    let paint = timing_values(performance, |timings| timings.paint);
    let accessibility = timing_values(performance, |timings| timings.accessibility);
    let other = timing_values(performance, |timings| timings.other);
    view! {
        <Scroll>
            <Frame padding=BODY_PADDING>
                <List spacing=PERFORMANCE_SPACING>
                    <List spacing=FOOTER_SPACING>
                        <Text style=TextStyle::CODE>{latest_work}</Text>
                        <Text style=TextStyle::CODE color=TextColor::Muted>{scene}</Text>
                        <Text style=TextStyle::CODE color=TextColor::Muted>{cache}</Text>
                        <Text style=TextStyle::CODE color=TextColor::Muted>{reuse}</Text>
                    </List>
                    <Separator />
                    <List spacing=TIMING_SPACING>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=TIMING_SPACING
                        >
                            <Text style=TextStyle::HEADING @sizing=ItemSize::Percent(100.0)>
                                "CPU time"
                            </Text>
                            <Text style=TextStyle::CAPTION>"milliseconds"</Text>
                        </List>
                        <TimingHeader />
                        <TimingRow label="Document" values={total} />
                        <TimingRow label="Layout" values={layout} />
                        <TimingRow label="Interaction" values={interaction} />
                        <TimingRow label="Paint" values={paint} />
                        <TimingRow label="Accessibility" values={accessibility} />
                        <TimingRow label="Other" values={other} />
                    </List>
                    <Separator />
                    <List spacing=TIMING_SPACING>
                        <Text style=TextStyle::HEADING>"Visualize"</Text>
                        <Checkbox
                            @test_id={"inspector.performance.flash_changes"}
                            label="Flash changed elements"
                            checked={Binding::new(state.value().flash_changes.get(), move |enabled| state.value().flash_changes.set(enabled))}
                        />
                        <Checkbox
                            @test_id={"inspector.performance.flash_damage"}
                            label="Flash repainted regions"
                            checked={Binding::new(state.value().flash_damage.get(), move |enabled| state.value().flash_damage.set(enabled))}
                        />
                    </List>
                    <Separator />
                    <RendererSection @test_id={"inspector.performance.renderer"} rows={renderer} />
                </List>
            </Frame>
        </Scroll>
    }
}

#[component]
fn RendererSection(rows: ReadSignal<RendererRows>) -> NodeId {
    let missing = create_memo(move || rows.with(Vec::is_empty));
    view! {
        <List spacing=TIMING_SPACING>
            <Text style=TextStyle::HEADING>"Renderer"</Text>
            <Show condition={missing}>
                <Text style=TextStyle::CAPTION>"Not reported by this host"</Text>
            </Show>
            <ForEach keys={rows}>|(label, value): (&'static str, String)|
                <LabeledRow label={label.to_owned()} value />
            </ForEach>
        </List>
    }
}

#[component]
fn PropertiesSection(properties: Memo<Vec<(String, String)>>) -> NodeId {
    view! {
        <List spacing=Space::None>
            <Separator />
            <Scroll @sizing=ItemSize::Percent(100.0)>
                <Frame padding=BODY_PADDING>
                    <List spacing=TIMING_SPACING>
                        <Text style=TextStyle::HEADING>"Properties"</Text>
                        <ForEach keys={properties}>|(label, value): (String, String)|
                            <LabeledRow label value />
                        </ForEach>
                    </List>
                </Frame>
            </Scroll>
        </List>
    }
}

#[component]
fn LabeledRow(label: String, value: String) -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=TIMING_SPACING>
            <Text style=TextStyle::CAPTION @sizing=ItemSize::Fixed(RENDERER_LABEL_WIDTH)>
                {label}
            </Text>
            <Text
                style=TextStyle::CAPTION
                @sizing=ItemSize::Percent(100.0)
                color=TextColor::Default
            >
                {value}
            </Text>
        </List>
    }
}

#[component]
fn TimingHeader() -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=TIMING_SPACING>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Text style=TextStyle::CAPTION @sizing=ItemSize::Fixed(54.0) align=TextAlign::End>
                "Current"
            </Text>
            <Text style=TextStyle::CAPTION @sizing=ItemSize::Fixed(54.0) align=TextAlign::End>
                "Average"
            </Text>
            <Text style=TextStyle::CAPTION @sizing=ItemSize::Fixed(54.0) align=TextAlign::End>
                "Peak"
            </Text>
        </List>
    }
}

#[component]
fn TimingRow(label: String, values: [Memo<String>; 3]) -> NodeId {
    let [current, average, peak] = values;
    view! {
        <List direction=Direction::Horizontal spacing=TIMING_SPACING>
            <Text
                overflow=Overflow::Ellipsis
                style=TextStyle::CAPTION
                @sizing=ItemSize::Percent(100.0)
            >
                {label}
            </Text>
            <Text style=TextStyle::CODE @sizing=ItemSize::Fixed(54.0) align=TextAlign::End>
                {current}
            </Text>
            <Text style=TextStyle::CODE @sizing=ItemSize::Fixed(54.0) align=TextAlign::End>
                {average}
            </Text>
            <Text style=TextStyle::CODE @sizing=ItemSize::Fixed(54.0) align=TextAlign::End>
                {peak}
            </Text>
        </List>
    }
}

fn performance_text(
    performance: ReadSignal<PerformanceSummary>,
    read: impl Fn(&PerformanceSummary) -> &String + 'static,
) -> Memo<String> {
    create_memo(move || performance.with(|summary| read(summary).clone()))
}

fn timing_values(
    performance: ReadSignal<PerformanceSummary>,
    read: impl Fn(PerformanceTimings) -> Duration + Clone + 'static,
) -> [Memo<String>; 3] {
    let timing = |select: fn(&PerformanceSummary) -> PerformanceTimings| {
        let read = read.clone();
        create_memo(move || format_duration(read(performance.with(select))))
    };
    [
        timing(|summary| summary.current),
        timing(|summary| summary.average),
        timing(|summary| summary.peak),
    ]
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}", duration.as_secs_f64() * 1_000.0)
}

fn percentage(count: usize, total: usize) -> usize {
    count.saturating_mul(100) / total.max(1)
}

fn entry_field<T: Clone + Default + PartialEq + 'static>(
    entries: &Entries,
    key: Key,
    read: impl Fn(&Entry) -> T + 'static,
) -> Memo<T> {
    let entries = *entries;
    create_memo(move || entries.with(|entries| entries.get(&key).map(&read).unwrap_or_default()))
}

#[component]
fn TreeCells(row_key: Key, entries: Entries) -> NodeId {
    let kind = entry_field(&entries, row_key, |entry| entry.kind.to_owned());
    let detail = entry_field(&entries, row_key, |entry| entry.detail.clone());
    let size = entry_field(&entries, row_key, |entry| entry.size.clone());

    view! {
        <List
            @test_id={row_key.test_id()}
            direction=Direction::Horizontal
            align=Align::Center
            spacing=ROW_SPACING
        >
            <Text style=TextStyle::CODE>{kind}</Text>
            <Text
                overflow=Overflow::Ellipsis
                style=TextStyle::CODE
                @sizing=ItemSize::Percent(100.0)
                color=TextColor::Muted
            >
                {detail}
            </Text>
            <Text style=TextStyle::CODE color=TextColor::Muted align=TextAlign::End>{size}</Text>
        </List>
    }
}

fn ancestors(selection: ReadSignal<Vec<Key>>, key: Key) -> Vec<Key> {
    selection.with(|path| match path.split_last() {
        Some((last, ancestors)) if *last == key => ancestors.to_vec(),
        _ => Vec::new(),
    })
}

fn item(entries: &Entries, key: Key) -> TreeItem {
    entries.with(|entries| {
        entries
            .get(&key)
            .map(|entry| TreeItem {
                label: entry_label(entry),
                depth: entry.depth,
                expandable: entry.expandable,
                expanded: entry.expanded,
            })
            .unwrap_or_default()
    })
}
