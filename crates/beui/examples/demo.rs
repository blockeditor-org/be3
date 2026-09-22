use beui::icons::ICON_GRID_VIEW;
use beui::reactive::{
    Align, Callback, Canvas, CanvasItem, CanvasView, ForEach, Frame, Keyed, List, Memo, ReadSignal,
    Selector, Show, Spacer, Text, WriteSignal, build, clone, create_memo, create_selector,
    create_signal, view,
};
use beui::styled::theme::{CARD_RADIUS, NARROW_WIDTH, RADIUS};
use beui::styled::{
    Accordion, Body, Button, ButtonVariant, Caption, Card, Checkbox, ContextMenu, Display, Heading,
    Link, Listbox, NumberInput, Paragraph, Progress, RadioGroup, ResponsiveTabs, Scroll, Select,
    Separator, Shortcut, Slider, Stack, Switch, TextArea, TextInput, Title, ToggleButton, Tree,
    TreeRowFace, VirtualList, use_theme,
};
use beui::unstyled::{
    ChoiceOption, Container, MAX_SCALE, MIN_SCALE, PanZoom, PanZoomHandle, PanZoomView,
    SliderScale, TextAreaState, TreeItem, narrower_than, shorter_than,
};
use beui::{Color32, Context, Direction, Document, ItemSize, NodeId, Rect, TextAlign, unstyled};
use beui_macros::component;
use std::sync::Arc;
use text_editor_core::{EditorCommand, MarkdownCommand, TextBuffer, TextLanguage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    beui::run("beui demo", DemoApp::new())
}

const HEADER_HEIGHT: f32 = 64.0;
const COMPACT_HEADER_HEIGHT: f32 = 52.0;
const HEADER_PADDING: f32 = 20.0;
const BODY_PADDING: f32 = 20.0;
const COMPACT_PADDING: f32 = 12.0;
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 3.0;
const ZOOM_MIDPOINT: f32 = 1.0;
const BODY_SPACING: f32 = 20.0;
const SHORT_HEIGHT: f32 = 900.0;
const CARD_NARROW_WIDTH: f32 = 460.0;
const TABS_NARROW_WIDTH: f32 = 380.0;
const ICON_BUTTON_WIDTH: f32 = 44.0;
const TOOLBAR_RULE_LENGTH: f32 = 20.0;
const ROW_COUNT: usize = 10_000;
const CRAMPED_ROWS_HEIGHT: f32 = 320.0;
const ROW_HEIGHT: f32 = 34.0;
const CRAMPED_EDITOR_HEIGHT: f32 = 260.0;
const EDITOR_SHARE: f32 = 40.0;
const ROWS_SHARE: f32 = 60.0;
const EDITOR_TEXT: &str = "# Notes\n\nA **multiline** editor with a gutter, wrapping and `markdown`\nstyling.\n\n- [x] click a checkbox\n- [ ] press Ctrl+F to find\n";
const COMPACT_ROW_HEIGHT: f32 = 25.0;
const TREE_NODES: [(&str, usize); 9] = [
    ("Project", 0),
    ("src", 1),
    ("main.rs", 2),
    ("demo.rs", 2),
    ("assets", 1),
    ("logo.png", 2),
    ("theme.toml", 2),
    ("README.md", 1),
    ("Cargo.toml", 1),
];
const STRIP_COUNT: usize = 200;
const STRIP_HEIGHT: f32 = 52.0;
const STRIP_ITEM_WIDTH: f32 = 120.0;
const STAGE_HEIGHT: f32 = 260.0;
const STAGE_VIEW: PanZoomView = PanZoomView::new(beui::pos2(200.0, 120.0), 0.8);
const STAGE_CARDS: [(f32, f32, f32, f32, &str); 4] = [
    (0.0, 0.0, 170.0, 90.0, "Inbox"),
    (230.0, 30.0, 150.0, 80.0, "Notes"),
    (50.0, 150.0, 210.0, 110.0, "Canvas"),
    (320.0, 190.0, 130.0, 70.0, "Archive"),
];
const STAGE_ZOOM_STEP: f32 = 1.3;
const STAGE_LABEL_SIZE: f32 = 15.0;
const STAGE_PADDING: f32 = 10.0;
const ROW_PADDING_HORIZONTAL: f32 = 12.0;
const ROW_PADDING_VERTICAL: f32 = 9.0;
const COMPACT_ROW_PADDING_VERTICAL: f32 = 4.0;

struct DemoApp {
    document: Document,
}

impl DemoApp {
    fn new() -> Self {
        let document = build(|| {
            let (count, set_count) = create_signal(0i64);
            let theme = use_theme();
            view! {
                <Frame color={theme.background.clone()} radius=0>
                    <Container>
                        {move |_| view! {
                            <DemoShell count set_count />
                        }}
                    </Container>
                </Frame>
            }
        });

        Self { document }
    }
}

impl beui::App for DemoApp {
    fn update(&mut self, context: &Context, rect: Rect) {
        self.document.show(context, rect);
    }

    fn clear_color(&self) -> Color32 {
        self.document.theme().background
    }
}

#[derive(Clone)]
struct Rows {
    set_status: WriteSignal<String>,
    selection: Selector<Option<usize>>,
    set_selected: WriteSignal<Option<usize>>,
    timings: ReadSignal<bool>,
    set_timings: WriteSignal<bool>,
    compact: ReadSignal<bool>,
    set_compact: WriteSignal<bool>,
}

impl Rows {
    fn new(set_status: WriteSignal<String>) -> Self {
        let (selected, set_selected) = create_signal(None);
        let selection = create_selector(clone!(selected -> move || selected.get()));
        let (timings, set_timings) = create_signal(true);
        let (compact, set_compact) = create_signal(false);
        Self {
            set_status,
            selection,
            set_selected,
            timings,
            set_timings,
            compact,
            set_compact,
        }
    }

    fn set_compact(&self, compact: bool) {
        self.set_compact.set(compact);
    }

    fn select(&self, index: usize) {
        self.set_selected.set(Some(index));
        self.set_status.set(format!("Row {index} selected"));
    }

    fn show_timings(&self, shown: bool) {
        self.set_timings.set(shown);
    }
}

#[component]
fn ScrollRow(index: usize, rows: Rows, compact: bool) -> NodeId {
    let selected = rows.selection.memo(Some(index));
    let select_rows = rows.clone();
    view! {
        <unstyled::Button
            on_click={move || select_rows.select(index)}
            content={move |handle| view! {
                <ScrollRowFace index handle selected timings={rows.timings} compact />
            }}
        />
    }
}

#[component]
fn ScrollRowFace(
    index: usize,
    handle: unstyled::ButtonHandle,
    selected: Memo<bool>,
    timings: ReadSignal<bool>,
    compact: bool,
) -> NodeId {
    let unstyled::ButtonHandle {
        hovered, focused, ..
    } = handle;
    let vertical = if compact {
        COMPACT_ROW_PADDING_VERTICAL
    } else {
        ROW_PADDING_VERTICAL
    };
    let theme = use_theme();
    let value_color = create_memo(clone!(selected theme -> move || {
        let theme = theme.get();
        if selected.get() {
            theme.accent
        } else {
            theme.text_muted
        }
    }));
    let fill_color = create_memo(clone!(theme -> move || {
        let theme = theme.get();
        match (selected.get(), hovered.get()) {
            (true, _) => theme.accent_soft,
            (false, true) => theme.hover,
            (false, false) => Color32::TRANSPARENT,
        }
    }));

    view! {
        <Frame
            color={fill_color}
            outline={theme.accent.clone()}
            outline_width=2.0
            radius=RADIUS
            outline_visible={focused}
            padding_horizontal=ROW_PADDING_HORIZONTAL
            padding_vertical={vertical}
        >
            <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                <Body @sizing=ItemSize::Percent(100.0) content={format!("Row {index}")} />
                <Frame visible={timings}>
                    <Caption
                        content={format!("{} ms", 7 + index * 3 % 91)}
                        align=TextAlign::End
                        color={value_color}
                    />
                </Frame>
            </List>
        </Frame>
    }
}

#[component]
fn DemoShell(count: ReadSignal<i64>, set_count: WriteSignal<i64>) -> NodeId {
    let narrow = narrower_than(NARROW_WIDTH);
    let header_height = create_memo(move || {
        if narrow.get() {
            ItemSize::Fixed(COMPACT_HEADER_HEIGHT)
        } else {
            ItemSize::Fixed(HEADER_HEIGHT)
        }
    });
    view! {
        <List spacing=0.0>
            <DemoHeader @sizing={header_height} set_count />
            <Separator />
            <DemoBody @sizing=ItemSize::Percent(100.0) count />
        </List>
    }
}

#[component]
fn DemoHeader(set_count: WriteSignal<i64>) -> NodeId {
    let reset_count = set_count.clone();
    let decrement_count = set_count.clone();
    let narrow = narrower_than(NARROW_WIDTH);
    let wide = create_memo(clone!(narrow -> move || !narrow.get()));
    let horizontal = create_memo(move || {
        if narrow.get() {
            COMPACT_PADDING
        } else {
            HEADER_PADDING
        }
    });
    let theme = use_theme();
    view! {
        <Frame color={theme.surface.clone()} padding_horizontal={horizontal}>
            <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=10.0>
                    <Title content="beui" />
                    <Frame visible={wide}>
                        <Caption content="retained mode ui" />
                    </Frame>
                </List>
                <Spacer @sizing=ItemSize::Percent(100.0) />
                <Button
                    label="Reset"
                    variant=ButtonVariant::Secondary
                    on_click={move || {
                        reset_count.set(0);
                    }}
                />
                <Button
                    @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH)
                    label="-"
                    variant=ButtonVariant::Primary
                    on_click={move || {
                        decrement_count.update(|value| *value = value.saturating_sub(1));
                    }}
                />
                <Button
                    @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH)
                    label="+"
                    variant=ButtonVariant::Primary
                    on_click={move || {
                        set_count.update(|value| *value = value.saturating_add(1));
                    }}
                />
            </List>
        </Frame>
    }
}

#[component]
fn DemoBody(count: ReadSignal<i64>) -> NodeId {
    let narrow = narrower_than(NARROW_WIDTH);
    let short = shorter_than(SHORT_HEIGHT);
    let cramped = create_memo(move || narrow.get() || short.get());
    view! {
        <List spacing=0.0>
            <Keyed value={cramped} key={|cramped: bool| cramped}>
                {move |value: ReadSignal<bool>| {
                    let (cramped, count) = (value.get_untracked(), count.clone());
                    view! {
                        <DemoPanels cramped count @sizing=ItemSize::Percent(100.0) />
                    }
                }}
            </Keyed>
        </List>
    }
}

#[component]
fn DemoPanels(cramped: bool, count: ReadSignal<i64>) -> NodeId {
    if !cramped {
        return view! {
            <PaddedPanels cramped count />
        };
    }
    view! {
        <Scroll>
            <PaddedPanels cramped count />
        </Scroll>
    }
}

#[component]
fn PaddedPanels(cramped: bool, count: ReadSignal<i64>) -> NodeId {
    let narrow = narrower_than(NARROW_WIDTH);
    let padding = create_memo(move || {
        if narrow.get() {
            COMPACT_PADDING
        } else {
            BODY_PADDING
        }
    });
    view! {
        <Frame padding_horizontal={padding.clone()} padding_vertical={padding}>
            <DemoPanelStack cramped count />
        </Frame>
    }
}

#[component]
fn DemoPanelStack(cramped: bool, count: ReadSignal<i64>) -> NodeId {
    view! {
        <Stack spacing=BODY_SPACING>
            <Sidebar @sizing=ItemSize::Percent(32.0) />
            <MainPanel @sizing=ItemSize::Percent(68.0) cramped count />
        </Stack>
    }
}

#[component]
fn Sidebar() -> NodeId {
    let narrow = narrower_than(NARROW_WIDTH);
    let open = create_memo(move || !narrow.get());
    let keyboard_open = open.clone();
    view! {
        <Card>
            <List spacing=12.0>
                <Accordion title="About" open>
                    <Paragraph
                        content="beui keeps a retained tree of nodes. Base nodes carry behaviour only, unstyled \
                         components compose them, and the styled components paint them."
                    />
                </Accordion>
                <Separator />
                <Accordion title="Keyboard" open={keyboard_open}>
                    <List spacing=12.0>
                        <Shortcut keys="Tab" description="move focus to the next control" />
                        <Shortcut keys="Shift+Tab" description="move focus back" />
                        <Shortcut keys="Enter" description="activate the focused control" />
                        <Shortcut
                            keys="Arrows"
                            description="adjust sliders or move within choices"
                        />
                        <Shortcut keys="Page Up/Down" description="scroll the focused row list" />
                        <Shortcut keys="Ctrl+Z" description="undo an edit in a text field" />
                        <Shortcut keys="Ctrl+Shift+I" description="open the inspector" />
                        <Shortcut keys="Ctrl+Shift+C" description="pick a node to inspect" />
                    </List>
                </Accordion>
            </List>
        </Card>
    }
}

#[component]
fn MainPanel(cramped: bool, count: ReadSignal<i64>) -> NodeId {
    let (status_text, set_status_text) = create_signal("Nothing selected".to_string());
    let rows = Rows::new(set_status_text);
    let compact = rows.compact.clone();
    let row_height = create_memo(move || {
        if compact.get() {
            COMPACT_ROW_HEIGHT
        } else {
            ROW_HEIGHT
        }
    });
    let item_rows = rows.clone();
    let (editor_size, rows_size) = if cramped {
        (
            ItemSize::Fixed(CRAMPED_EDITOR_HEIGHT),
            ItemSize::Fixed(CRAMPED_ROWS_HEIGHT),
        )
    } else {
        (
            ItemSize::Percent(EDITOR_SHARE),
            ItemSize::Percent(ROWS_SHARE),
        )
    };

    view! {
        <List spacing=20.0>
            <Card>
                <List spacing=4.0>
                    <Caption content="Counter" />
                    <Display
                        content={create_memo(clone!(count -> move || count.get().to_string()))}
                    />
                    <Paragraph
                        content="Click the header buttons, or focus one with Tab and press Enter."
                    />
                </List>
            </Card>
            <EditorCard @sizing={editor_size} />
            <Controls rows={rows.clone()} />
            <CanvasCard @sizing=ItemSize::Fixed(STAGE_HEIGHT) />
            <Card @sizing={rows_size}>
                <List spacing=12.0>
                    <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                        <Heading content={format!("Rows ({ROW_COUNT})")} />
                        <Caption
                            @sizing=ItemSize::Percent(100.0)
                            content={status_text}
                            align=TextAlign::End
                        />
                    </List>
                    <Separator />
                    <VirtualList
                        @sizing=ItemSize::Percent(100.0)
                        count=ROW_COUNT
                        item_size={row_height}
                    >
                        {move |index: usize| {
                            let rows = item_rows.clone();
                            let compact = rows.compact.get();
                            view! {
                                <ScrollRow index rows compact />
                            }
                        }}
                    </VirtualList>
                </List>
            </Card>
        </List>
    }
}

#[component]
fn EditorCard() -> NodeId {
    let document = Arc::new(TextBuffer::new(EDITOR_TEXT)) as Arc<dyn text_editor_core::Document>;
    let state = TextAreaState::new(document);
    state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
    let bold = state.clone();
    let italic = state.clone();
    view! {
        <Card>
            <List spacing=12.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                    <Heading @sizing=ItemSize::Percent(100.0) content="Editor" />
                    <Button
                        label="Bold"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            bold.execute(EditorCommand::Markdown(MarkdownCommand::Bold));
                        }}
                    />
                    <Button
                        label="Italic"
                        variant=ButtonVariant::Secondary
                        on_click={move || {
                            italic.execute(EditorCommand::Markdown(MarkdownCommand::Italic));
                        }}
                    />
                </List>
                <TextArea @sizing=ItemSize::Percent(100.0) state={state} />
            </List>
        </Card>
    }
}

#[component]
fn CanvasCard() -> NodeId {
    let (stage_view, set_stage_view) = create_signal(STAGE_VIEW);
    let zoom_out = set_stage_view.clone();
    let zoom_in = set_stage_view.clone();
    let stage = set_stage_view.clone();
    let zoom_label = create_memo(clone!(stage_view -> move || {
        format!("{:.0}%", stage_view.get().scale * 100.0)
    }));

    view! {
        <Card>
            <List spacing=12.0>
                <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                    <Heading @sizing=ItemSize::Percent(100.0) content="Canvas" />
                    <Caption content={zoom_label} />
                    <Separator direction=Direction::Vertical length=TOOLBAR_RULE_LENGTH />
                    <Button
                        @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH)
                        label="-"
                        variant=ButtonVariant::Secondary
                        on_click={move || zoom_out.update(scale_out)}
                    />
                    <Button
                        @sizing=ItemSize::Fixed(ICON_BUTTON_WIDTH)
                        label="+"
                        variant=ButtonVariant::Secondary
                        on_click={move || zoom_in.update(scale_in)}
                    />
                    <Button
                        label="Reset"
                        variant=ButtonVariant::Secondary
                        on_click={move || set_stage_view.set(STAGE_VIEW)}
                    />
                </List>
                <Separator />
                <CanvasStage
                    @sizing=ItemSize::Percent(100.0)
                    view={stage_view}
                    on_change={move |view| stage.set(view)}
                />
                <Caption
                    wrap=true
                    content="Scroll to pan, Shift+scroll sideways, Ctrl+scroll or pinch to zoom, \
                     and drag with the middle button. Tab to it for arrows, + and -."
                />
            </List>
        </Card>
    }
}

fn scale_out(view: &mut PanZoomView) {
    view.scale = (view.scale / STAGE_ZOOM_STEP).clamp(MIN_SCALE, MAX_SCALE);
}

fn scale_in(view: &mut PanZoomView) {
    view.scale = (view.scale * STAGE_ZOOM_STEP).clamp(MIN_SCALE, MAX_SCALE);
}

#[component]
fn CanvasStage(view: ReadSignal<PanZoomView>, on_change: Callback<PanZoomView>) -> NodeId {
    view! {
        <PanZoom view on_change={move |view| on_change.call(view)}>
            {move |handle: PanZoomHandle| {
                let PanZoomHandle { view, scale, focused, .. } = handle;
                view! {
                    <CanvasBoard view scale focused />
                }
            }}
        </PanZoom>
    }
}

#[component]
fn CanvasBoard(
    view: Memo<Option<CanvasView>>,
    scale: Memo<f32>,
    focused: ReadSignal<bool>,
) -> NodeId {
    let [first, second, third, fourth] = STAGE_CARDS;
    let theme = use_theme();
    view! {
        <Frame
            outline={theme.accent.clone()}
            outline_width=2.0
            outline_visible={focused}
            radius=RADIUS
        >
            <Canvas view>
                <StageCard card=first scale={scale.clone()} />
                <StageCard card=second scale={scale.clone()} />
                <StageCard card=third scale={scale.clone()} />
                <StageCard card=fourth scale />
            </Canvas>
        </Frame>
    }
}

#[component]
fn StageCard(card: (f32, f32, f32, f32, &'static str), scale: Memo<f32>) -> CanvasItem {
    let (x, y, width, height, label) = card;
    let theme = use_theme();
    let font_size = create_memo(clone!(scale -> move || STAGE_LABEL_SIZE * scale.get()));
    let padding = create_memo(clone!(scale -> move || STAGE_PADDING * scale.get()));

    view! {
        <CanvasItem x y width height>
            <Frame
                color={theme.surface_raised.clone()}
                outline={theme.border.clone()}
                outline_width=1.0
                outline_visible=true
                radius=CARD_RADIUS
                padding_horizontal={padding.clone()}
                padding_vertical={padding}
            >
                <Text string={label.to_string()} font_size={font_size} color={theme.text.clone()} />
            </Frame>
        </CanvasItem>
    }
}

#[component]
fn Controls(rows: Rows) -> NodeId {
    view! {
        <Card>
            <Container>
                {move |_| view! {
                    <ControlPanels rows />
                }}
            </Container>
        </Card>
    }
}

#[component]
fn ControlPanels(rows: Rows) -> NodeId {
    let (selected_tab, set_selected_tab) = create_signal(0usize);

    let tab = create_selector(clone!(selected_tab -> move || selected_tab.get()));
    let list_rows = rows.clone();

    view! {
        <List spacing=16.0>
            <ResponsiveTabs
                options={view! {
                    <ChoiceOption label="List" />
                    <ChoiceOption label="Strip" />
                    <ChoiceOption label="Load" />
                    <ChoiceOption label="Name" />
                    <ChoiceOption label="Links" />
                    <ChoiceOption label="Choices" />
                    <ChoiceOption label="Menus" />
                    <ChoiceOption label="Tree" />
                }}
                selected=0
                breakpoint=TABS_NARROW_WIDTH
                on_change={move |selected| {
                    set_selected_tab.set(selected);
                }}
            />
            <List spacing=0.0>
                <Show condition={tab.memo(0)}>
                    <ListControls rows=list_rows />
                </Show>
                <Show condition={tab.memo(1)}>
                    <StripControls />
                </Show>
                <Show condition={tab.memo(2)}>
                    <LoadControls />
                </Show>
                <Show condition={tab.memo(3)}>
                    <NameControls />
                </Show>
                <Show condition={tab.memo(4)}>
                    <LinkControls />
                </Show>
                <Show condition={tab.memo(5)}>
                    <ChoiceControls />
                </Show>
                <Show condition={tab.memo(6)}>
                    <MenuControls />
                </Show>
                <Show condition={tab.memo(7)}>
                    <TreeControls />
                </Show>
            </List>
        </List>
    }
}

#[component]
fn ListControls(rows: Rows) -> NodeId {
    let timing_rows = rows.clone();
    let compact_rows = rows.clone();
    view! {
        <List spacing=12.0>
            <Checkbox
                label="Show timings"
                checked=true
                on_change={move |checked| {
                    timing_rows.show_timings(checked);
                }}
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                <Switch on=false on_change={move |on| compact_rows.set_compact(on)} />
                <Body @sizing=ItemSize::Percent(100.0) content="Compact rows" />
            </List>
        </List>
    }
}

#[component]
fn StripControls() -> NodeId {
    view! {
        <List spacing=12.0>
            <Paragraph
                content="A horizontal list scrolls with Shift+scroll, a sideways trackpad swipe, \
                 a touch drag, or the arrow keys once something in it has focus."
            />
            <VirtualList
                @sizing=ItemSize::Fixed(STRIP_HEIGHT)
                direction=Direction::Horizontal
                count=STRIP_COUNT
                item_size=STRIP_ITEM_WIDTH
            >
                {move |index: usize| view! {
                    <StripCard index />
                }}
            </VirtualList>
        </List>
    }
}

#[component]
fn StripCard(index: usize) -> NodeId {
    let theme = use_theme();
    view! {
        <Frame padding_horizontal=4.0 padding_vertical=0.0>
            <Frame
                color={theme.surface_raised.clone()}
                outline={theme.border.clone()}
                outline_width=1.0
                outline_visible=true
                radius=RADIUS
                padding_horizontal=12.0
                padding_vertical=10.0
            >
                <Body content={format!("Card {index}")} align=TextAlign::Center />
            </Frame>
        </Frame>
    }
}

#[component]
fn LoadControls() -> NodeId {
    let (progress_value, set_progress_value) = create_signal(0.4f32);
    let readout_value = progress_value.clone();
    let (zoom, set_zoom) = create_signal(1.0f32);
    let zoom_readout = zoom.clone();

    view! {
        <List spacing=12.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                <Caption content="Simulated load" />
                <Caption
                    @sizing=ItemSize::Percent(100.0)
                    content={create_memo(move || percent_label(readout_value.get()))}
                    align=TextAlign::End
                />
            </List>
            <Slider
                value=0.4
                on_change={move |value| {
                    set_progress_value.set(value);
                }}
            />
            <Progress value={progress_value} />
            <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                <Caption content="Zoom" />
                <Caption
                    @sizing=ItemSize::Percent(100.0)
                    content={create_memo(move || format!("{:.2}x", zoom_readout.get()))}
                    align=TextAlign::End
                />
            </List>
            <Slider
                value={zoom}
                min=ZOOM_MIN
                max=ZOOM_MAX
                scale={SliderScale::Midpoint(ZOOM_MIDPOINT)}
                label="Zoom"
                on_change={move |value| set_zoom.set(value)}
            />
        </List>
    }
}

#[component]
fn LinkControls() -> NodeId {
    let (followed, set_followed) = create_signal("Nothing followed yet".to_owned());
    let set_docs = set_followed.clone();

    view! {
        <List spacing=12.0>
            <Paragraph
                content="A link reads as a link rather than a button, underlines itself while \
                 hovered or focused, and takes Space or Enter like any other control."
            />
            <List direction=Direction::Horizontal align=Align::Center spacing=16.0>
                <Link
                    label="Open the grid"
                    glyph={ICON_GRID_VIEW.to_owned()}
                    on_click={move || set_followed.set("Followed: Open the grid".to_owned())}
                />
                <Link
                    label="Read the guide"
                    on_click={move || set_docs.set("Followed: Read the guide".to_owned())}
                />
                <Link label="Unavailable" disabled=true />
                <Spacer @sizing=ItemSize::Percent(100.0) />
            </List>
            <Caption content={followed} />
        </List>
    }
}

#[component]
fn NameControls() -> NodeId {
    let (greeting_text, set_greeting_text) = create_signal(greeting_label(""));
    let (seats, set_seats) = create_signal(4.0f64);
    let seats_text = create_memo(clone!(seats -> move || format!("{} seats", seats.get())));
    let (locked, set_locked) = create_signal(true);

    view! {
        <List spacing=12.0>
            <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                <Caption content="Display name" />
                <Caption
                    @sizing=ItemSize::Percent(100.0)
                    content={greeting_text}
                    align=TextAlign::End
                />
            </List>
            <TextInput
                value=String::new()
                placeholder="Type a name"
                on_change={move |value| {
                    set_greeting_text.set(greeting_label(&value));
                }}
            />
            <Paragraph content="Click to place the caret, drag to select, and Ctrl+Z to undo." />
            <Separator />
            <List direction=Direction::Horizontal align=Align::Center spacing=12.0>
                <Caption content="Seats" />
                <Caption
                    @sizing=ItemSize::Percent(100.0)
                    content={seats_text}
                    align=TextAlign::End
                />
            </List>
            <NumberInput
                value={seats}
                min=1.0
                max=12.0
                label="Seats"
                on_change={move |value| set_seats.set(value)}
            />
            <Separator />
            <Switch label="Locked" on={locked.clone()} on_change={move |on| set_locked.set(on)} />
            <TextInput value="Read only while locked" disabled={locked.clone()} />
            <Select
                options={view! {
                    <ChoiceOption label="String" />
                    <ChoiceOption label="Number" />
                }}
                selected=Some(0)
                label="Field type"
                disabled={locked.clone()}
            />
            <Checkbox label="Minimum" checked=false disabled={locked.clone()} />
            <Button label="Save" variant=ButtonVariant::Primary disabled={locked} />
        </List>
    }
}

fn greeting_label(name: &str) -> String {
    if name.is_empty() {
        "Nobody yet".to_owned()
    } else {
        format!("Hello, {name}")
    }
}

fn percent_label(value: f32) -> String {
    format!("{}%", (value * 100.0).round())
}

#[component]
fn ChoiceControls() -> NodeId {
    let modes = ["Automatic", "Manual", "Scheduled"];
    let (mode_status_text, set_mode_status_text) = create_signal("Automatic updates".to_string());
    let colors = ["Amber", "Blue", "Green", "Purple"];
    let (color_status_text, set_color_status_text) = create_signal("Blue selected".to_string());
    let (pin_status_text, set_pin_status_text) = create_signal("Selection is unpinned".to_string());

    view! {
        <Stack spacing=20.0 breakpoint=CARD_NARROW_WIDTH>
            <List @sizing=ItemSize::Percent(50.0) spacing=8.0>
                <Caption content="Update mode" />
                <RadioGroup
                    options={view! {
                        <ChoiceOption label="Automatic" />
                        <ChoiceOption label="Manual" />
                        <ChoiceOption label="Scheduled" />
                    }}
                    selected=Some(0)
                    on_change={move |selected| {
                        if let Some(index) = selected {
                            let text = format!("{} updates", modes[index]);
                            set_mode_status_text.set(text);
                        }
                    }}
                />
                <Caption content={mode_status_text} />
                <ToggleButton
                    label="Pin selection"
                    pressed=false
                    on_change={move |pressed| {
                        let text = if pressed {
                            "Selection is pinned"
                        } else {
                            "Selection is unpinned"
                        }
                        .to_string();
                        set_pin_status_text.set(text);
                    }}
                />
                <Caption content={pin_status_text} />
            </List>
            <List @sizing=ItemSize::Percent(50.0) spacing=8.0>
                <Caption content="Highlight color (type to search)" />
                <Listbox
                    options={view! {
                        <ChoiceOption label="Amber" />
                        <ChoiceOption label="Blue" />
                        <ChoiceOption label="Green" />
                        <ChoiceOption label="Purple" />
                    }}
                    selected=Some(1)
                    on_change={move |selected| {
                        if let Some(index) = selected {
                            let text = format!("{} selected", colors[index]);
                            set_color_status_text.set(text);
                        }
                    }}
                />
                <Caption content={color_status_text} />
            </List>
        </Stack>
    }
}

#[component]
fn TreeControls() -> NodeId {
    let (collapsed, set_collapsed) = create_signal(Vec::<usize>::new());
    let (selected, set_selected) = create_signal(None::<usize>);
    let keys = create_memo(clone!(collapsed -> move || visible_tree_rows(&collapsed.get())));
    let status_text = create_memo(clone!(selected -> move || match selected.get() {
        Some(row) => format!("{} selected", TREE_NODES[row].0),
        None => "Nothing selected".to_owned(),
    }));
    let expansion = collapsed.clone();

    view! {
        <List spacing=8.0>
            <Caption
                content="Arrow keys walk the tree; Enter or a click opens and closes a folder"
            />
            <Tree
                keys
                item={move |row: usize| tree_item(row, &expansion.get())}
                selected
                on_select={move |row: usize| set_selected.set(Some(row))}
                on_expand={move |(row, expanded): (usize, bool)| {
                    set_collapsed.update(|collapsed| {
                        collapsed.retain(|candidate| *candidate != row);
                        if !expanded {
                            collapsed.push(row);
                        }
                    });
                }}
            >
                {move |row: TreeRowFace<usize>| view! {
                    <Body content={TREE_NODES[row.key].0} />
                }}
            </Tree>
            <Caption content={status_text} />
        </List>
    }
}

fn tree_item(row: usize, collapsed: &[usize]) -> TreeItem {
    let (label, depth) = TREE_NODES[row];
    TreeItem {
        label: label.to_owned(),
        depth,
        expandable: has_children(row),
        expanded: !collapsed.contains(&row),
        marked: false,
    }
}

fn has_children(row: usize) -> bool {
    TREE_NODES
        .get(row + 1)
        .is_some_and(|(_, depth)| *depth > TREE_NODES[row].1)
}

fn visible_tree_rows(collapsed: &[usize]) -> Vec<usize> {
    let mut rows = Vec::new();
    let mut hidden_below: Option<usize> = None;
    for (row, (_, depth)) in TREE_NODES.iter().enumerate() {
        match hidden_below {
            Some(limit) if *depth > limit => continue,
            _ => hidden_below = None,
        }
        rows.push(row);
        if collapsed.contains(&row) {
            hidden_below = Some(*depth);
        }
    }
    rows
}

const FRUITS: [&str; 6] = ["Apple", "Banana", "Cherry", "Date", "Grape", "Mango"];

#[component]
fn MenuControls() -> NodeId {
    let (fruit_status_text, set_fruit_status_text) = create_signal("Apple selected".to_string());
    let (menu_status_text, set_menu_status_text) = create_signal("Nothing chosen yet".to_string());

    let (copied, set_copied) = create_signal(false);
    let nothing_copied = create_memo(move || !copied.get());
    let items = view! {
        <unstyled::MenuItem label="Copy" />
        <unstyled::MenuItem label="Paste" disabled={nothing_copied} />
        <unstyled::MenuItem label="Share">
            <unstyled::MenuItem label="Email" />
            <unstyled::MenuItem label="Link" />
        </unstyled::MenuItem>
    };

    view! {
        <Stack spacing=20.0 breakpoint=CARD_NARROW_WIDTH>
            <List @sizing=ItemSize::Percent(50.0) spacing=8.0>
                <Caption content="Favorite fruit (type to search)" />
                <Select
                    options={view! {
                        <ForEach keys={FRUITS.to_vec()}>
                            {|label: &'static str| view! {
                                <ChoiceOption label />
                            }}
                        </ForEach>
                    }}
                    selected=Some(0)
                    on_change={move |selected| {
                        let text = selected
                            .and_then(|index| FRUITS.get(index))
                            .map_or_else(
                                || "Nothing selected".to_owned(),
                                |label| format!("{label} selected"),
                            );
                        set_fruit_status_text.set(text);
                    }}
                />
                <Caption content={fruit_status_text} />
            </List>
            <List @sizing=ItemSize::Percent(50.0) spacing=8.0>
                <ContextMenu
                    items
                    on_select={move |path: Vec<usize>| {
                        let label = match path.as_slice() {
                            [0] => {
                                set_copied.set(true);
                                "Copy".to_owned()
                            }
                            [1] => "Paste".to_owned(),
                            [2, 0] => "Share > Email".to_owned(),
                            [2, 1] => "Share > Link".to_owned(),
                            other => format!("{other:?}"),
                        };
                        set_menu_status_text.set(format!("Chose: {label}"));
                    }}
                >
                    <Card>
                        <List spacing=4.0>
                            <Caption content="Right-click the card below" />
                            <Paragraph
                                content="Paste stays disabled until Copy is chosen; Share opens a submenu on hover or Right Arrow, and Left Arrow closes it."
                            />
                        </List>
                    </Card>
                </ContextMenu>
                <Caption content={menu_status_text} />
            </List>
        </Stack>
    }
}
