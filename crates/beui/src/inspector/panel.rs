use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use accesskit::{Node, Role};

use crate::color::Color32;

use crate::base::{ScrollPosition, TextAlign};
use crate::document::Document;
use crate::filter::{ColorVision, MAX_BLUR};
use crate::icons::ICON_CLOSE;
use crate::node::NodeId;
use crate::reactive::{
    Align, Children, Direction, Frame, ItemSize, List, ListChild, Memo, NodeRef, Prop, ReadSignal,
    Scroll, Show, Spacer, WriteSignal, clone, component, create_memo, create_signal, view,
};
use crate::screen_reader::Command;
use crate::styled::theme::{BORDER_WIDTH, CHIP_RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT};
use crate::styled::{
    Button, ButtonVariant, Caption, Checkbox, Code, Heading, IconSized, RadioGroup, Scrollbar,
    Separator, Slider, Tabs, Theme, Tree,
};
use crate::unstyled;
use crate::unstyled::{ChoiceOption, SliderScale, TreeItem};

use super::tree::{Entry, Key};
use super::{InspectorTab, State, entry_label};
use crate::{PerformanceSnapshot, PerformanceTimings};

const HEADER_PADDING: f32 = 12.0;
const HEADER_SPACING: f32 = 8.0;
const BODY_PADDING: f32 = 8.0;
const BODY_SPACING: f32 = 6.0;
const FOOTER_PADDING: f32 = 10.0;
const FOOTER_SPACING: f32 = 3.0;
const ROW_SPACING: f32 = 6.0;
const TOGGLE_PADDING_HORIZONTAL: f32 = 8.0;
const TOGGLE_PADDING_VERTICAL: f32 = 3.0;
const CLOSE_PADDING: f32 = 3.0;
const CLOSE_ICON_SIZE: f32 = 16.0;
const PERFORMANCE_SPACING: f32 = 10.0;
const TIMING_SPACING: f32 = 4.0;
const BLUR_MIDPOINT: f32 = 12.0;
const PIXEL_RATIOS: [(&str, Option<f32>); 5] = [
    ("Native", None),
    ("1x", Some(1.0)),
    ("1.5x", Some(1.5)),
    ("2x", Some(2.0)),
    ("3x", Some(3.0)),
];
const THEMES: [(&str, Theme); 2] = [("Dark", Theme::DARK), ("E-ink", Theme::EINK)];
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
const KEYBOARD_GUIDE: [&str; 7] = [
    "Arrows - previous or next item",
    "Tab and Shift+Tab - previous or next control",
    "Enter or Space - activate",
    "Home and End - first or last item",
    "Minus and Plus - adjust the value",
    "Page Up and Page Down - scroll",
    "R - repeat",
];
const TOUCH_GUIDE: [&str; 5] = [
    "Drag a finger - read what is under it",
    "Flick sideways - previous or next item",
    "Flick up or down - adjust the value",
    "Double tap - activate",
    "Two finger tap - repeat, drag two fingers - scroll",
];
const THEME: Theme = Theme::DARK;

#[derive(Clone, Default, PartialEq)]
pub(crate) struct Summary {
    pub(crate) total: usize,
    pub(crate) native_pixel_ratio: String,
    pub(crate) picking: bool,
    pub(crate) selection: String,
    pub(crate) bounds: String,
}

#[derive(Clone, Default, PartialEq)]
pub(crate) struct PerformanceSummary {
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
                    "Reused: {} of {} measured | {} of {} painted",
                    work.reused_measurements,
                    work.reused_measurements + work.measured,
                    work.replayed_nodes,
                    work.replayed_nodes + work.painted_nodes
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

type Entries = ReadSignal<HashMap<Key, Entry>>;

pub(crate) struct Panel {
    pub(crate) document: Document,
    pub(crate) set_keys: WriteSignal<Vec<Key>>,
    pub(crate) set_entries: WriteSignal<HashMap<Key, Entry>>,
    pub(crate) set_summary: WriteSignal<Summary>,
    pub(crate) set_performance: WriteSignal<PerformanceSummary>,
    pub(crate) set_selection: WriteSignal<Option<Key>>,
    pub(crate) set_reveal: WriteSignal<Option<Key>>,
    pub(crate) tree: NodeRef,
}

pub(crate) fn build(state: &Rc<State>) -> Panel {
    let (keys, set_keys) = create_signal(Vec::<Key>::new());
    let (entries, set_entries) = create_signal(HashMap::<Key, Entry>::new());
    let (summary, set_summary) = create_signal(Summary::default());
    let (performance, set_performance) = create_signal(PerformanceSummary::default());
    let (tab, set_tab) = create_signal(InspectorTab::default());
    let (position, set_position) = create_signal(ScrollPosition::ZERO);
    let (selection, set_selection) = create_signal(None);
    let (reveal, set_reveal) = create_signal(None);
    let tree = NodeRef::new();
    let tree_ref = tree.clone();
    let simulation_state = state.clone();
    let performance_state = state.clone();
    let tab_state = state.clone();
    let reset_state = state.clone();
    let close_state = state.clone();

    let mut document = crate::reactive::build(|| {
        let count_text = create_memo({
            let (summary, performance, tab) = (summary.clone(), performance.clone(), tab.clone());
            move || match tab.get() {
                InspectorTab::Performance => {
                    performance.with(|performance| performance.samples.clone())
                }
                InspectorTab::Simulation => String::new(),
                _ => total_label(summary.with(|summary| summary.total)),
            }
        });
        let tree_visible = create_memo({
            let tab = tab.clone();
            move || matches!(tab.get(), InspectorTab::Beui | InspectorTab::AccessKit)
        });
        let performance_visible = create_memo({
            let tab = tab.clone();
            move || tab.get() == InspectorTab::Performance
        });
        let simulation_visible = create_memo(move || tab.get() == InspectorTab::Simulation);
        let native_pixel_ratio_text = create_memo({
            let summary = summary.clone();
            move || summary.with(|summary| summary.native_pixel_ratio.clone())
        });
        let picking = create_memo({
            let summary = summary.clone();
            move || summary.with(|summary| summary.picking)
        });
        let selection_text = create_memo({
            let summary = summary.clone();
            move || summary.with(|summary| summary.selection.clone())
        });
        let bounds_text = create_memo(move || summary.with(|summary| summary.bounds.clone()));
        let (select_state, expand_state, hover_state) =
            (state.clone(), state.clone(), state.clone());
        let row_entries = entries.clone();
        let item_entries = entries.clone();
        let pick_state = state.clone();
        let header_tree_visible = tree_visible.clone();
        let body_tree_visible = tree_visible.clone();
        let footer_tree_visible = tree_visible;
        let body_performance_visible = performance_visible.clone();
        let footer_performance_visible = performance_visible;
        let body_simulation_visible = simulation_visible.clone();
        let footer_simulation_visible = simulation_visible;
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                <Frame @sizing=ItemSize::Percent(100.0) color={THEME.surface} radius=0>
                    <List spacing=0.0>
                        <Frame padding_horizontal=HEADER_PADDING padding_vertical=HEADER_PADDING>
                            <List spacing=HEADER_SPACING>
                                <List
                                    direction=Direction::Horizontal
                                    align=Align::Center
                                    spacing=HEADER_SPACING
                                >
                                    <Heading content="Inspector" />
                                    <Caption
                                        @sizing=ItemSize::Percent(100.0)
                                        content={count_text}
                                        align=TextAlign::End
                                    />
                                    <Show condition={header_tree_visible}>
                                        <PickToggle state={pick_state} picking />
                                    </Show>
                                    <CloseButton @test_id={"inspector.close"} state={close_state} />
                                </List>
                                <Tabs
                                    @test_id={"inspector.tabs"}
                                    options={view! {
                                        <ChoiceOption label="Beui" />
                                        <ChoiceOption label="A11y" />
                                        <ChoiceOption label="Perf" />
                                        <ChoiceOption label="Sim" />
                                    }}
                                    selected=0
                                    on_change={move |index| {
                                        tab_state.set_tab(index);
                                        set_tab.set(InspectorTab::from_index(index));
                                    }}
                                />
                            </List>
                        </Frame>
                        <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                        <Frame
                            @sizing=ItemSize::Percent(100.0)
                            padding_horizontal=BODY_PADDING
                            padding_vertical=BODY_PADDING
                        >
                            <List spacing=0.0>
                                <Show condition={body_tree_visible}>
                                    <List
                                        @sizing=ItemSize::Percent(100.0)
                                        direction=Direction::Horizontal
                                        spacing=BODY_SPACING
                                    >
                                        <Scroll
                                            @sizing=ItemSize::Percent(100.0)
                                            focus_color={THEME.accent}
                                            on_change={move |value| set_position.set(value)}
                                        >
                                            <Tree
                                                @node_ref=&tree_ref
                                                keys
                                                item={move |key: Key| item(&item_entries, key)}
                                                selected={selection}
                                                reveal
                                                on_select={move |key: Key| select_state.select(key.node())}
                                                on_expand={move |(key, expanded): (Key, bool)| {
                                                    expand_state.set_expanded(key, expanded);
                                                }}
                                                on_hover_change={move |(key, hovered): (Key, bool)| {
                                                    hover_state.hover(key.node(), hovered);
                                                }}
                                            >
                                                {move |key: Key| view! {
                                                    <TreeCells
                                                        row_key={key}
                                                        entries={row_entries.clone()}
                                                    />
                                                }}
                                            </Tree>
                                        </Scroll>
                                        <Scrollbar
                                            @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH)
                                            position
                                        />
                                    </List>
                                </Show>
                                <Show condition={body_performance_visible}>
                                    <PerformancePanel
                                        @sizing=ItemSize::Percent(100.0)
                                        @test_id={"inspector.performance"}
                                        performance={performance.clone()}
                                        state={performance_state.clone()}
                                    />
                                </Show>
                                <Show condition={body_simulation_visible}>
                                    <SimulationPanel
                                        @sizing=ItemSize::Percent(100.0)
                                        state={simulation_state.clone()}
                                    />
                                </Show>
                            </List>
                        </Frame>
                        <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                        <Frame padding_horizontal=FOOTER_PADDING padding_vertical=FOOTER_PADDING>
                            <List spacing=0.0>
                                <Show condition={footer_tree_visible}>
                                    <List spacing=FOOTER_SPACING>
                                        <Code content={selection_text} />
                                        <Code content={bounds_text} color={THEME.text_muted} />
                                    </List>
                                </Show>
                                <Show condition={footer_performance_visible}>
                                    <Button
                                        label="Reset samples"
                                        variant=ButtonVariant::Secondary
                                        on_click={move || reset_state.reset_performance()}
                                    />
                                </Show>
                                <Show condition={footer_simulation_visible}>
                                    <Code
                                        content={native_pixel_ratio_text}
                                        color={THEME.text_muted}
                                    />
                                </Show>
                            </List>
                        </Frame>
                    </List>
                </Frame>
            </List>
        }
    });
    document.inspectable = false;

    Panel {
        document,
        set_keys,
        set_entries,
        set_summary,
        set_performance,
        set_selection,
        set_reveal,
        tree,
    }
}

#[component]
fn SimulationPanel(state: Rc<State>) -> NodeId {
    let (position, set_position) = create_signal(ScrollPosition::ZERO);
    let simulated = state.simulated_pixels_per_point.get();
    let selected = PIXEL_RATIOS
        .iter()
        .position(|(_, ratio)| *ratio == simulated)
        .unwrap_or(0);
    let ratio_options = PIXEL_RATIOS
        .iter()
        .map(|(label, _)| {
            view! {
                <ChoiceOption label={*label} />
            }
        })
        .collect::<Vec<_>>();
    let theme = state.theme.get();
    let selected_theme = THEMES
        .iter()
        .position(|(_, candidate)| *candidate == theme)
        .unwrap_or(0);
    let theme_options = THEMES
        .iter()
        .map(|(label, _)| {
            view! {
                <ChoiceOption label={*label} />
            }
        })
        .collect::<Vec<_>>();
    let (touch_state, mouse_state, ratio_state, theme_state) =
        (state.clone(), state.clone(), state.clone(), state.clone());
    let reader_state = state.clone();
    let filter_state = state.clone();
    view! {
        <List direction=Direction::Horizontal spacing=BODY_SPACING>
            <Scroll
                @sizing=ItemSize::Percent(100.0)
                focus_color={THEME.accent}
                on_change={move |value| set_position.set(value)}
            >
                <List spacing=PERFORMANCE_SPACING>
                    <Checkbox
                        @test_id={"inspector.simulation.touch_emulation"}
                        label="Emulate touch with mouse"
                        checked={state.touch_emulation.get()}
                        on_change={move |enabled| touch_state.touch_emulation.set(enabled)}
                    />
                    <Checkbox
                        @test_id={"inspector.simulation.mouse_simulation"}
                        label="Simulate mouse with touch"
                        checked={state.mouse_simulation.get()}
                        on_change={move |enabled| mouse_state.mouse_simulation.set(enabled)}
                    />
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <List spacing=TIMING_SPACING>
                        <Heading content="Device pixel ratio" />
                        <RadioGroup
                            @test_id={"inspector.simulation.pixel_ratio"}
                            options={ratio_options}
                            selected={Some(selected)}
                            on_change={move |index: Option<usize>| {
                                let ratio = index.and_then(|index| PIXEL_RATIOS.get(index));
                                ratio_state.simulate_pixels_per_point(ratio.and_then(|(_, ratio)| *ratio));
                            }}
                        />
                    </List>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <List spacing=TIMING_SPACING>
                        <Heading content="Theme" />
                        <RadioGroup
                            @test_id={"inspector.simulation.theme"}
                            options={theme_options}
                            selected={Some(selected_theme)}
                            on_change={move |index: Option<usize>| {
                                if let Some((_, theme)) = index.and_then(|index| THEMES.get(index)) {
                                    theme_state.choose_theme(*theme);
                                }
                            }}
                        />
                    </List>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <FilterSection state={filter_state} />
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <ScreenReaderSection state={reader_state} />
                </List>
            </Scroll>
            <Scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position />
        </List>
    }
}

#[component]
fn FilterSection(state: Rc<State>) -> NodeId {
    let (blur_state, contrast_state, vision_state) = (state.clone(), state.clone(), state.clone());
    let (blur_text, set_blur_text) = create_signal(blur_label(state.blur.get()));
    let (contrast_text, set_contrast_text) =
        create_signal(contrast_label(state.contrast_reduction.get()));
    let vision = state.color_vision.get();
    let selected_vision = ColorVision::ALL
        .iter()
        .position(|(_, candidate)| *candidate == vision)
        .unwrap_or(0);
    let vision_options = ColorVision::ALL
        .iter()
        .map(|(label, _)| {
            view! {
                <ChoiceOption label={*label} />
            }
        })
        .collect::<Vec<_>>();
    view! {
        <List spacing=TIMING_SPACING>
            <Heading content="Filters" />
            <Caption content={blur_text} />
            <Slider
                @test_id={"inspector.simulation.blur"}
                label="Blur"
                value={state.blur.get()}
                min=0.0
                max=MAX_BLUR
                scale={SliderScale::Midpoint(BLUR_MIDPOINT)}
                on_change={move |value| {
                    blur_state.set_blur(value);
                    set_blur_text.set(blur_label(value));
                }}
            />
            <Caption content={contrast_text} />
            <Slider
                @test_id={"inspector.simulation.contrast"}
                label="Reduce contrast"
                value={state.contrast_reduction.get()}
                on_change={move |value| {
                    contrast_state.set_contrast_reduction(value);
                    set_contrast_text.set(contrast_label(value));
                }}
            />
            <Caption content="Colour vision" />
            <RadioGroup
                @test_id={"inspector.simulation.color_vision"}
                options={vision_options}
                selected={Some(selected_vision)}
                on_change={move |index: Option<usize>| {
                    if let Some((_, vision)) = index.and_then(|index| ColorVision::ALL.get(index)) {
                        vision_state.choose_color_vision(*vision);
                    }
                }}
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
fn ScreenReaderSection(state: Rc<State>) -> NodeId {
    let enable_state = state.clone();
    let (first_state, second_state, third_state) = (state.clone(), state.clone(), state.clone());
    let (fourth_state, fifth_state) = (state.clone(), state.clone());
    view! {
        <List spacing=TIMING_SPACING>
            <Heading content="Screen reader" />
            <Checkbox
                @test_id={"inspector.screen_reader.enabled"}
                label="Simulate a screen reader"
                checked={state.screen_reader.get()}
                on_change={move |enabled| enable_state.enable_screen_reader(enabled)}
            />
            <CommandRow row=0 state />
            <CommandRow row=1 state={first_state} />
            <CommandRow row=2 state={second_state} />
            <CommandRow row=3 state={third_state} />
            <CommandRow row=4 state={fourth_state} />
            <CommandRow row=5 state={fifth_state} />
            <GuideSection title="Keyboard" lines={KEYBOARD_GUIDE.as_slice()} />
            <GuideSection title="Touch" lines={TOUCH_GUIDE.as_slice()} />
        </List>
    }
}

#[component]
fn GuideSection(title: &'static str, lines: &'static [&'static str]) -> NodeId {
    let guides: Children<ListChild> = lines
        .iter()
        .map(|line| {
            view! {
                <Caption content={(*line).to_owned()} wrap=true />
            }
        })
        .collect::<Vec<_>>()
        .into();
    view! {
        <List spacing=FOOTER_SPACING>
            <Caption content={title.to_owned()} color={THEME.text} />
            {guides}
        </List>
    }
}

#[component]
fn CommandRow(row: usize, state: Rc<State>) -> NodeId {
    let (left_label, left_id, left) = COMMANDS[row * 2];
    let (right_label, right_id, right) = COMMANDS[row * 2 + 1];
    let right_state = state.clone();
    view! {
        <List direction=Direction::Horizontal spacing=TIMING_SPACING>
            <Button
                @sizing=ItemSize::Percent(100.0)
                @test_id={format!("inspector.screen_reader.{left_id}")}
                label={left_label.to_owned()}
                variant=ButtonVariant::Secondary
                on_click={move || state.command(left)}
            />
            <Button
                @sizing=ItemSize::Percent(100.0)
                @test_id={format!("inspector.screen_reader.{right_id}")}
                label={right_label.to_owned()}
                variant=ButtonVariant::Secondary
                on_click={move || right_state.command(right)}
            />
        </List>
    }
}

pub(crate) fn total_label(total: usize) -> String {
    match total {
        1 => "1 node".to_owned(),
        total => format!("{total} nodes"),
    }
}

#[component]
fn PerformancePanel(performance: ReadSignal<PerformanceSummary>, state: Rc<State>) -> NodeId {
    let (position, set_position) = create_signal(ScrollPosition::ZERO);
    let (change_state, damage_state) = (state.clone(), state.clone());
    let latest_work = performance_text(&performance, |summary| &summary.latest_work);
    let scene = performance_text(&performance, |summary| &summary.scene);
    let cache = performance_text(&performance, |summary| &summary.cache);
    let reuse = performance_text(&performance, |summary| &summary.reuse);
    let total = timing_values(&performance, |timings| timings.total);
    let layout = timing_values(&performance, |timings| timings.layout);
    let interaction = timing_values(&performance, |timings| timings.interaction);
    let paint = timing_values(&performance, |timings| timings.paint);
    let accessibility = timing_values(&performance, |timings| timings.accessibility);
    let other = timing_values(&performance, |timings| timings.other);
    view! {
        <List direction=Direction::Horizontal spacing=BODY_SPACING>
            <Scroll
                @sizing=ItemSize::Percent(100.0)
                focus_color={THEME.accent}
                on_change={move |value| set_position.set(value)}
            >
                <List spacing=PERFORMANCE_SPACING>
                    <List spacing=FOOTER_SPACING>
                        <Code content={latest_work} />
                        <Code content={scene} color={THEME.text_muted} />
                        <Code content={cache} color={THEME.text_muted} />
                        <Code content={reuse} color={THEME.text_muted} />
                    </List>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <List spacing=TIMING_SPACING>
                        <List
                            direction=Direction::Horizontal
                            align=Align::Center
                            spacing=TIMING_SPACING
                        >
                            <Heading @sizing=ItemSize::Percent(100.0) content="CPU time" />
                            <Caption content="milliseconds" />
                        </List>
                        <TimingHeader />
                        <TimingRow label="Document" values={total} />
                        <TimingRow label="Layout" values={layout} />
                        <TimingRow label="Interaction" values={interaction} />
                        <TimingRow label="Paint" values={paint} />
                        <TimingRow label="Accessibility" values={accessibility} />
                        <TimingRow label="Other" values={other} />
                    </List>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <List spacing=TIMING_SPACING>
                        <Heading content="Visualize" />
                        <Checkbox
                            @test_id={"inspector.performance.flash_changes"}
                            label="Flash changed elements"
                            checked={state.flash_changes.get()}
                            on_change={move |enabled| change_state.flash_changes.set(enabled)}
                        />
                        <Checkbox
                            @test_id={"inspector.performance.flash_damage"}
                            label="Flash repainted regions"
                            checked={state.flash_damage.get()}
                            on_change={move |enabled| damage_state.flash_damage.set(enabled)}
                        />
                    </List>
                </List>
            </Scroll>
            <Scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position />
        </List>
    }
}

#[component]
fn TimingHeader() -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=TIMING_SPACING>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Caption @sizing=ItemSize::Fixed(54.0) content="Current" align=TextAlign::End />
            <Caption @sizing=ItemSize::Fixed(54.0) content="Average" align=TextAlign::End />
            <Caption @sizing=ItemSize::Fixed(54.0) content="Peak" align=TextAlign::End />
        </List>
    }
}

#[component]
fn TimingRow(label: String, values: [Memo<String>; 3]) -> NodeId {
    let [current, average, peak] = values;
    view! {
        <List direction=Direction::Horizontal spacing=TIMING_SPACING>
            <Caption @sizing=ItemSize::Percent(100.0) content={label} />
            <Code @sizing=ItemSize::Fixed(54.0) content={current} align=TextAlign::End />
            <Code @sizing=ItemSize::Fixed(54.0) content={average} align=TextAlign::End />
            <Code @sizing=ItemSize::Fixed(54.0) content={peak} align=TextAlign::End />
        </List>
    }
}

fn performance_text(
    performance: &ReadSignal<PerformanceSummary>,
    read: impl Fn(&PerformanceSummary) -> &String + 'static,
) -> Memo<String> {
    let performance = performance.clone();
    create_memo(move || performance.with(|summary| read(summary).clone()))
}

fn timing_values(
    performance: &ReadSignal<PerformanceSummary>,
    read: impl Fn(PerformanceTimings) -> Duration + Clone + 'static,
) -> [Memo<String>; 3] {
    let timing = |select: fn(&PerformanceSummary) -> PerformanceTimings| {
        let performance = performance.clone();
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

pub(crate) fn toggle_fill(picking: bool) -> Color32 {
    if picking {
        THEME.accent
    } else {
        THEME.surface_raised
    }
}

pub(crate) fn toggle_text(picking: bool) -> Color32 {
    if picking { THEME.on_accent } else { THEME.text }
}

#[component]
fn PickToggle(state: Rc<State>, picking: Memo<bool>) -> NodeId {
    let label_color = create_memo(clone!(picking -> move || toggle_text(picking.get())));
    let fill_color = create_memo(move || toggle_fill(picking.get()));
    let picker = state;
    view! {
        <unstyled::Pressable on_click={move || picker.toggle_picking()}>
            <Frame
                color={fill_color}
                outline={THEME.border}
                outline_width=BORDER_WIDTH
                radius=CHIP_RADIUS
                outline_visible=true
                padding_horizontal=TOGGLE_PADDING_HORIZONTAL
                padding_vertical=TOGGLE_PADDING_VERTICAL
            >
                <Code content="Pick" align=TextAlign::Center color={label_color} />
            </Frame>
        </unstyled::Pressable>
    }
}

#[component]
fn CloseButton(state: Rc<State>) -> NodeId {
    let (hovered, set_hovered) = create_signal(false);
    let (focused, set_focused) = create_signal(false);
    let fill_color = create_memo(move || close_fill(hovered.get()));
    let outline_color = create_memo(move || close_outline(focused.get()));
    let closer = state;
    let mut accessibility = Node::new(Role::Button);
    accessibility.set_label("Close inspector");
    view! {
        <unstyled::Pressable
            accessibility={Prop::Static(accessibility)}
            on_click={move || closer.close()}
            on_hover_change={move |hovered| set_hovered.set(hovered)}
            on_focus_change={move |focused| set_focused.set(focused)}
        >
            <Frame
                color={fill_color}
                outline={outline_color}
                outline_width=BORDER_WIDTH
                radius=CHIP_RADIUS
                outline_visible=true
                padding_horizontal=CLOSE_PADDING
                padding_vertical=CLOSE_PADDING
            >
                <IconSized glyph=ICON_CLOSE font_size=CLOSE_ICON_SIZE color={THEME.text_muted} />
            </Frame>
        </unstyled::Pressable>
    }
}

fn close_fill(hovered: bool) -> Color32 {
    if hovered {
        THEME.pressed
    } else {
        Color32::TRANSPARENT
    }
}

fn close_outline(focused: bool) -> Color32 {
    if focused { THEME.accent } else { THEME.border }
}

fn entry_field<T: Clone + Default + PartialEq + 'static>(
    entries: &Entries,
    key: Key,
    read: impl Fn(&Entry) -> T + 'static,
) -> Memo<T> {
    let entries = entries.clone();
    create_memo(move || entries.with(|entries| entries.get(&key).map(&read).unwrap_or_default()))
}

#[component]
fn TreeCells(row_key: Key, entries: Entries) -> NodeId {
    let key = row_key;
    let kind = entry_field(&entries, key, |entry| entry.kind.to_owned());
    let detail = entry_field(&entries, key, |entry| entry.detail.clone());
    let size = entry_field(&entries, key, |entry| entry.size.clone());

    view! {
        <List
            @test_id={key.test_id()}
            direction=Direction::Horizontal
            align=Align::Center
            spacing=ROW_SPACING
        >
            <Code content={kind} />
            <Code @sizing=ItemSize::Percent(100.0) content={detail} color={THEME.text_muted} />
            <Code content={size} color={THEME.text_muted} align=TextAlign::End />
        </List>
    }
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
                marked: false,
            })
            .unwrap_or_default()
    })
}
