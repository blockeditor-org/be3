use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use crate::color::Color32;

use crate::base::{ScrollPosition, TextAlign};
use crate::document::Document;
use crate::node::NodeId;
use crate::reactive::{
    CenteredRow, Column, Frame, ItemSize, Memo, NodeRef, ReadSignal, Row, Scroll, Show, Spacer,
    WriteSignal, clone, component, create_memo, create_signal, view,
};
use crate::styled::theme::{BORDER_WIDTH, CHIP_RADIUS, SCROLLBAR_WIDTH, SEPARATOR_HEIGHT};
use crate::styled::{
    Button, ButtonVariant, Caption, Checkbox, Code, Heading, RadioGroup, Scrollbar, Separator,
    Tabs, Theme, Tree,
};
use crate::unstyled;
use crate::unstyled::TreeItem;

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
const PERFORMANCE_SPACING: f32 = 10.0;
const TIMING_SPACING: f32 = 4.0;
const PIXEL_RATIOS: [(&str, Option<f32>); 5] = [
    ("Native", None),
    ("1x", Some(1.0)),
    ("1.5x", Some(1.5)),
    ("2x", Some(2.0)),
    ("3x", Some(3.0)),
];
const THEMES: [(&str, Theme); 2] = [("Dark", Theme::DARK), ("E-ink", Theme::EINK)];
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
}

impl From<PerformanceSnapshot> for PerformanceSummary {
    fn from(snapshot: PerformanceSnapshot) -> Self {
        let samples = match snapshot.samples {
            1 => "1 sample".to_owned(),
            count => format!("{count} samples"),
        };
        let latest = snapshot.latest;
        let (latest_work, scene, cache) = if snapshot.samples == 0 {
            (
                "Waiting for a document update".to_owned(),
                String::new(),
                "Cache: waiting for measurements".to_owned(),
            )
        } else {
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
            <Row spacing=0.0>
                <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                <Frame @sizing=ItemSize::Percent(100.0) color={THEME.surface} radius=0>
                    <Column spacing=0.0>
                        <Frame padding_horizontal=HEADER_PADDING padding_vertical=HEADER_PADDING>
                            <Column spacing=HEADER_SPACING>
                                <CenteredRow spacing=HEADER_SPACING>
                                    <Heading content="Inspector" />
                                    <Caption
                                        @sizing=ItemSize::Percent(100.0)
                                        content={count_text}
                                        align=TextAlign::End
                                    />
                                    <Show condition={header_tree_visible}>
                                        <PickToggle state={pick_state} picking />
                                    </Show>
                                </CenteredRow>
                                <Tabs
                                    @test_id={"inspector.tabs"}
                                    labels={vec!["Beui".to_owned(), "A11y".to_owned(), "Perf".to_owned(), "Sim".to_owned()]}
                                    selected=0
                                    on_change={move |index| {
                                        tab_state.set_tab(index);
                                        set_tab.set(InspectorTab::from_index(index));
                                    }}
                                />
                            </Column>
                        </Frame>
                        <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                        <Frame
                            @sizing=ItemSize::Percent(100.0)
                            padding_horizontal=BODY_PADDING
                            padding_vertical=BODY_PADDING
                        >
                            <Column spacing=0.0>
                                <Show
                                    @sizing=ItemSize::Percent(100.0)
                                    condition={body_tree_visible}
                                >
                                    <Row spacing=BODY_SPACING>
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
                                    </Row>
                                </Show>
                                <Show
                                    @sizing=ItemSize::Percent(100.0)
                                    condition={body_performance_visible}
                                >
                                    <PerformancePanel
                                        @test_id={"inspector.performance"}
                                        performance={performance.clone()}
                                        state={performance_state.clone()}
                                    />
                                </Show>
                                <Show
                                    @sizing=ItemSize::Percent(100.0)
                                    condition={body_simulation_visible}
                                >
                                    <SimulationPanel state={simulation_state.clone()} />
                                </Show>
                            </Column>
                        </Frame>
                        <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                        <Frame padding_horizontal=FOOTER_PADDING padding_vertical=FOOTER_PADDING>
                            <Column spacing=0.0>
                                <Show condition={footer_tree_visible}>
                                    <Column spacing=FOOTER_SPACING>
                                        <Code content={selection_text} />
                                        <Code content={bounds_text} color={THEME.text_muted} />
                                    </Column>
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
                            </Column>
                        </Frame>
                    </Column>
                </Frame>
            </Row>
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
    let labels = PIXEL_RATIOS
        .iter()
        .map(|(label, _)| (*label).to_owned())
        .collect::<Vec<_>>();
    let theme = state.theme.get();
    let selected_theme = THEMES
        .iter()
        .position(|(_, candidate)| *candidate == theme)
        .unwrap_or(0);
    let theme_labels = THEMES
        .iter()
        .map(|(label, _)| (*label).to_owned())
        .collect::<Vec<_>>();
    let (touch_state, mouse_state, ratio_state, theme_state) =
        (state.clone(), state.clone(), state.clone(), state.clone());
    view! {
        <Row spacing=BODY_SPACING>
            <Scroll
                @sizing=ItemSize::Percent(100.0)
                focus_color={THEME.accent}
                on_change={move |value| set_position.set(value)}
            >
                <Column spacing=PERFORMANCE_SPACING>
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
                    <Column spacing=TIMING_SPACING>
                        <Heading content="Device pixel ratio" />
                        <RadioGroup
                            @test_id={"inspector.simulation.pixel_ratio"}
                            labels
                            selected={Some(selected)}
                            on_change={move |index: Option<usize>| {
                                let ratio = index.and_then(|index| PIXEL_RATIOS.get(index));
                                ratio_state.simulate_pixels_per_point(ratio.and_then(|(_, ratio)| *ratio));
                            }}
                        />
                    </Column>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <Column spacing=TIMING_SPACING>
                        <Heading content="Theme" />
                        <RadioGroup
                            @test_id={"inspector.simulation.theme"}
                            labels={theme_labels}
                            selected={Some(selected_theme)}
                            on_change={move |index: Option<usize>| {
                                if let Some((_, theme)) = index.and_then(|index| THEMES.get(index)) {
                                    theme_state.choose_theme(*theme);
                                }
                            }}
                        />
                    </Column>
                </Column>
            </Scroll>
            <Scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position />
        </Row>
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
    let total = timing_values(&performance, |timings| timings.total);
    let layout = timing_values(&performance, |timings| timings.layout);
    let interaction = timing_values(&performance, |timings| timings.interaction);
    let paint = timing_values(&performance, |timings| timings.paint);
    let accessibility = timing_values(&performance, |timings| timings.accessibility);
    let other = timing_values(&performance, |timings| timings.other);
    view! {
        <Row spacing=BODY_SPACING>
            <Scroll
                @sizing=ItemSize::Percent(100.0)
                focus_color={THEME.accent}
                on_change={move |value| set_position.set(value)}
            >
                <Column spacing=PERFORMANCE_SPACING>
                    <Column spacing=FOOTER_SPACING>
                        <Code content={latest_work} />
                        <Code content={scene} color={THEME.text_muted} />
                        <Code content={cache} color={THEME.text_muted} />
                    </Column>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <Column spacing=TIMING_SPACING>
                        <CenteredRow spacing=TIMING_SPACING>
                            <Heading @sizing=ItemSize::Percent(100.0) content="CPU time" />
                            <Caption content="milliseconds" />
                        </CenteredRow>
                        <TimingHeader />
                        <TimingRow label="Document" values={total} />
                        <TimingRow label="Layout" values={layout} />
                        <TimingRow label="Interaction" values={interaction} />
                        <TimingRow label="Paint" values={paint} />
                        <TimingRow label="Accessibility" values={accessibility} />
                        <TimingRow label="Other" values={other} />
                    </Column>
                    <Separator @sizing=ItemSize::Fixed(SEPARATOR_HEIGHT) />
                    <Column spacing=TIMING_SPACING>
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
                    </Column>
                </Column>
            </Scroll>
            <Scrollbar @sizing=ItemSize::Fixed(SCROLLBAR_WIDTH) position />
        </Row>
    }
}

#[component]
fn TimingHeader() -> NodeId {
    view! {
        <Row spacing=TIMING_SPACING>
            <Spacer @sizing=ItemSize::Percent(100.0) />
            <Caption @sizing=ItemSize::Fixed(54.0) content="Current" align=TextAlign::End />
            <Caption @sizing=ItemSize::Fixed(54.0) content="Average" align=TextAlign::End />
            <Caption @sizing=ItemSize::Fixed(54.0) content="Peak" align=TextAlign::End />
        </Row>
    }
}

#[component]
fn TimingRow(label: String, values: [Memo<String>; 3]) -> NodeId {
    let [current, average, peak] = values;
    view! {
        <Row spacing=TIMING_SPACING>
            <Caption @sizing=ItemSize::Percent(100.0) content={label} />
            <Code @sizing=ItemSize::Fixed(54.0) content={current} align=TextAlign::End />
            <Code @sizing=ItemSize::Fixed(54.0) content={average} align=TextAlign::End />
            <Code @sizing=ItemSize::Fixed(54.0) content={peak} align=TextAlign::End />
        </Row>
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
        <CenteredRow @test_id={key.test_id()} spacing=ROW_SPACING>
            <Code content={kind} />
            <Code @sizing=ItemSize::Percent(100.0) content={detail} color={THEME.text_muted} />
            <Code content={size} color={THEME.text_muted} align=TextAlign::End />
        </CenteredRow>
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
            })
            .unwrap_or_default()
    })
}
