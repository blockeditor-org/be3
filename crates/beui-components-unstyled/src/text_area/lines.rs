use std::rc::Rc;

use beui_macros::{component, view};

use beui_core::base::ItemSize;
use beui_core::base::overlay::{OverlayAnchor, OverlayMode, Placement};
use beui_core::color::Color32;
use beui_core::font::{FontId, TextAlign};
use beui_core::geometry::Vec2;
use beui_core::icons::{ICON_CHECK, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT};
use beui_core::node::NodeId;
use beui_core::rich::{TextCaret, TextMark};
use beui_view::components::overlay::Overlay;
use beui_view::reactive::{
    Canvas, CanvasItem, ForEach, Frame, List, Memo, NodeRef, Portal, RenderFn, Show, Text,
    TextItem, VirtualList, clone, component_rect, create_effect, create_memo, on_cleanup, untrack,
    with_document,
};

use super::rows::{
    BODY_SIZE, CHECKBOX_WIDTH, DOCUMENT_PADDING, INLINE_WIDGET_HEIGHT, INLINE_WIDGET_ICON_INSET,
    Inline, InlineItem, LINE_PADDING, Row, RowInputs, RowOptions, build_row, galley, line_range,
    rich_layout,
};
use super::{
    CARET_WIDTH, CHECKBOX_OUTLINE, CHECKBOX_RADIUS, CODE_OUTSET, CODE_RADIUS, Context,
    GUTTER_ARROW_SIZE, GUTTER_PADDING_LEFT, GUTTER_PADDING_RIGHT, GUTTER_TEXT_SIZE, GeometryCell,
    INLINE_WIDGET_RADIUS, REMOTE_SELECTION_ALPHA, RowEntry,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Layer {
    All,
    Text,
    Handles,
}

#[component]
pub(super) fn Lines(cx: Context) -> NodeId {
    let canvas = cx.state.canvas();
    let fill = create_memo(clone!(cx -> move || Some(cx.view_height.get())));
    let list = cx.list.clone();
    let keys = cx.visible.clone();
    let top = create_memo(clone!(cx -> move || Some(cx.padding.get().y)));
    let bottom =
        create_memo(clone!(cx -> move || Some((DOCUMENT_PADDING.y - cx.padding.get().y).max(0.0))));
    let estimate = cx.row_estimate();
    view! {
        <Frame @node_ref=&canvas min_height={fill}>
            <List spacing=0.0>
                <Frame height={top} />
                <VirtualList @node_ref=&list keys item_size=estimate>
                    {move |line: usize| view! {
                        <AreaRow cx={cx.clone()} line />
                    }}
                </VirtualList>
                <Frame height={bottom} />
            </List>
        </Frame>
    }
}

#[component]
pub(super) fn SingleLine(cx: Context) -> NodeId {
    let canvas = cx.state.canvas();
    let model = row_model(&cx, 0);
    let text = create_memo(clone!(cx model -> move || {
        let row = model.get();
        cx.metrics.get();
        let size = cx.font_size.get();
        let body = galley("", FontId::proportional(size)).map_or(0.0, |galley| galley.line_height());
        let laid = rich_layout(&row, size, (0.0, 0.0), f32::INFINITY)
            .map_or(Vec2::ZERO, |layout| layout.size);
        Vec2::new(laid.x, laid.y.max(body))
    }));
    let canvas_width = create_memo(clone!(cx text -> move || {
        text.get().x + cx.padding.get().x * 2.0 + CARET_WIDTH
    }));
    let canvas_height =
        create_memo(clone!(cx text -> move || text.get().y + cx.padding.get().y * 2.0));
    let x = create_memo(clone!(cx -> move || cx.padding.get().x - cx.shift.get()));
    let y = create_memo(clone!(cx text -> move || {
        let padding = cx.padding.get();
        let line = text.get().y;
        let height = cx.view_height.get();
        match height > line + padding.y * 2.0 {
            true => (height - line) / 2.0,
            false => padding.y,
        }
    }));
    let text_width = create_memo(clone!(text -> move || text.get().x + CARET_WIDTH));
    let text_height = create_memo(clone!(text -> move || text.get().y));
    let (text_cx, text_model) = (cx.clone(), model.clone());
    view! {
        <Canvas @node_ref=&canvas width={canvas_width} height={canvas_height}>
            <CanvasItem
                x={x.clone()}
                y={y.clone()}
                width={text_width.clone()}
                height={text_height.clone()}
            >
                <AreaText cx={text_cx} model={text_model} />
            </CanvasItem>
            <CanvasItem x y width={text_width} height={text_height} clip=false>
                <RowText cx model text={GeometryCell::default()} wrap=false layer=Layer::Handles />
            </CanvasItem>
        </Canvas>
    }
}

#[component]
fn AreaRow(cx: Context, line: usize) -> NodeId {
    let model = row_model(&cx, line);
    let fill = create_memo(clone!(cx model -> move || {
        let start = model.get().start;
        let revealed = cx.state.content();
        revealed.get();
        match cx.state.with_snapshot(|snapshot| {
            snapshot
                .sections
                .iter()
                .any(|section| section.revealed && start > section.line_end && start <= section.content_end)
        }) {
            true => cx.colors.get().revealed_background,
            false => Color32::TRANSPARENT,
        }
    }));
    let gutter = create_memo(clone!(cx -> move || Some(cx.gutter.get())));
    let padding = create_memo(clone!(cx -> move || (cx.padding.get().x - CODE_OUTSET.x).max(0.0)));
    let inset = create_memo(clone!(cx -> move || cx.padding.get().x.min(CODE_OUTSET.x)));
    let code = create_memo(clone!(cx model -> move || match model.get().code_block {
        true => cx.colors.get().code_background,
        false => Color32::TRANSPARENT,
    }));
    let block = create_memo(clone!(model -> move || model.get().block));
    let renders = cx.block.is_some();
    let has_block = create_memo(clone!(block -> move || renders && block.get().is_some()));
    let block_ref = NodeRef::new();
    let block_cx = cx.clone();
    let block_node = block_ref.clone();
    let row = NodeRef::new();
    let text = GeometryCell::default();
    register_row(&cx, line, &row, &text, &block_ref, &model);
    let (gutter_cx, gutter_model) = (cx.clone(), model.clone());
    view! {
        <Frame @node_ref=&row color={fill}>
            <List direction=beui_core::base::Direction::Horizontal spacing=0.0>
                <Frame width={gutter}>
                    <Gutter cx={gutter_cx} model={gutter_model} />
                </Frame>
                <Frame @sizing=ItemSize::Percent(100.0) padding_horizontal={padding}>
                    <Frame color={code} padding_horizontal={inset}>
                        <List spacing=0.0>
                            <RowText cx model text wrap=true />
                            <Show condition={has_block}>
                                {move || {
                                    let (index, size) = block.get_untracked().unwrap_or_default();
                                    let render = block_cx
                                        .block
                                        .clone()
                                        .expect("a block is only shown when the area was given one");
                                    let node = block_node.clone();
                                    view! {
                                        <BlockSlot @node_ref=&node render index size />
                                    }
                                }}
                            </Show>
                        </List>
                    </Frame>
                </Frame>
            </List>
        </Frame>
    }
}

#[component]
fn AreaText(cx: Context, model: Memo<Rc<Row>>) -> NodeId {
    let row = NodeRef::new();
    let text = GeometryCell::default();
    register_row(&cx, 0, &row, &text, &NodeRef::new(), &model);
    view! {
        <Frame @node_ref=&row>
            <RowText cx model text wrap=false layer=Layer::Text />
        </Frame>
    }
}

fn register_row(
    cx: &Context,
    line: usize,
    row: &NodeRef,
    geometry: &GeometryCell,
    block: &NodeRef,
    model: &Memo<Rc<Row>>,
) {
    let id = cx.registered.get() + 1;
    cx.registered.set(id);
    cx.rows.borrow_mut().insert(
        line,
        RowEntry {
            id,
            row: row.clone(),
            geometry: geometry.clone(),
            model: model.clone(),
            block: block.clone(),
        },
    );
    let registry = cx.clone();
    on_cleanup(move || {
        let mut rows = registry.rows.borrow_mut();
        if rows.get(&line).is_some_and(|entry| entry.id == id) {
            rows.remove(&line);
        }
    });
}

fn row_model(cx: &Context, line: usize) -> Memo<Rc<Row>> {
    let cx = cx.clone();
    let content = cx.state.content();
    let cursors = cx.state.cursors();
    create_memo(move || {
        content.get();
        cursors.get();
        cx.metrics.get();
        let starts = cx.starts.get();
        let tables = cx.tables.get();
        let widgets = cx.widgets.get();
        let colors = cx.colors.get();
        let composition = cx.composition.get();
        let placeholder = cx.placeholder.get();
        let options = RowOptions {
            body_size: cx.font_size.get(),
            mask: cx.masked.get(),
            single_line: cx.single_line,
        };
        let selection = cx.state.selection_ranges();
        Rc::new(cx.state.with_snapshot(|snapshot| {
            let (start, end, newline) =
                line_range(&snapshot.bytes, &starts, line).unwrap_or((0, 0, false));
            let spacers = tables.get(&start).map_or(&[][..], Vec::as_slice);
            let placeholder = (snapshot.bytes.is_empty() && !placeholder.is_empty() && line == 0)
                .then_some(placeholder.as_str());
            if !snapshot.loaded || snapshot.highlight.is_none() {
                return Row {
                    line,
                    start,
                    end,
                    ..Row::default()
                };
            }
            build_row(
                &RowInputs {
                    snapshot,
                    widgets: &widgets,
                    composition: composition.as_ref(),
                    selection: &selection,
                    colors: &colors,
                    options,
                    spacers,
                    placeholder,
                },
                line,
                start,
                end,
                newline,
            )
        }))
    })
}

#[component]
fn RowText(
    cx: Context,
    model: Memo<Rc<Row>>,
    text: GeometryCell,
    wrap: bool,
    #[prop(default = Layer::All)] layer: Layer,
) -> NodeId {
    let node = NodeRef::new();
    let placed = component_rect();
    create_effect(clone!(node text -> move || {
        placed.get();
        if layer != Layer::Handles
            && text.borrow().is_none()
            && let Some(id) = node.try_get()
        {
            *text.borrow_mut() = Some(with_document(|document| document.text_geometry(id)));
        }
    }));
    let display = create_memo(clone!(model -> move || model.get().display.clone()));
    let spans = create_memo(clone!(model -> move || {
        let mut spans = model.get().spans.clone();
        if layer == Layer::Handles {
            for span in &mut spans {
                span.style.color = Color32::TRANSPARENT;
                span.style.underline = false;
                span.style.strikethrough = false;
            }
        }
        spans
    }));
    let inline = create_memo(clone!(model -> move || match layer {
        Layer::Handles => Vec::new(),
        _ => (0..model.get().inline.len()).collect::<Vec<usize>>(),
    }));
    let padding = match cx.single_line {
        true => (0.0, 0.0),
        false => LINE_PADDING,
    };
    let marks = create_memo(clone!(cx model -> move || {
        if layer == Layer::Handles {
            return Vec::new();
        }
        let row = model.get();
        let colors = cx.colors.get();
        cx.state.cursors().get();
        let mut marks: Vec<TextMark> = row
            .code
            .iter()
            .map(|range| TextMark {
                range: range.clone(),
                color: colors.code_background,
                radius: CODE_RADIUS,
                outset: CODE_OUTSET,
            })
            .collect();
        for range in cx.state.selection_ranges() {
            if range.start < range.end && range.start <= row.end && range.end > row.start {
                marks.push(TextMark::new(row.display_range(&range), colors.selection));
            }
        }
        for remote in cx.remote_cursors.get() {
            let range = remote.selection;
            if range.start < range.end && range.start <= row.end && range.end > row.start {
                let [red, green, blue, _] = remote.color.to_array();
                let fill = Color32::from_rgba_unmultiplied(red, green, blue, REMOTE_SELECTION_ALPHA);
                marks.push(TextMark::new(row.display_range(&range), fill));
            }
        }
        marks
    }));
    let carets = create_memo(clone!(cx model -> move || {
        let row = model.get();
        let colors = cx.colors.get();
        let focused = cx.focused.get();
        cx.state.cursors().get();
        let contains = |byte: usize| byte >= row.start && byte <= row.end;
        let mut carets = Vec::new();
        let touch = cx.state.touch_mode().get();
        cx.state.caret_handle().get();
        if touch && layer != Layer::Text {
            cx.shift.get();
            for (handle, byte) in untrack(|| cx.handles()) {
                if contains(byte) {
                    carets.push(TextCaret {
                        handle: Some(handle),
                        ..TextCaret::new(row.to_display(byte), colors.caret, CARET_WIDTH)
                    });
                }
            }
        }
        if layer == Layer::Handles {
            return carets;
        }
        for remote in cx.remote_cursors.get() {
            if contains(remote.caret) {
                carets.push(TextCaret {
                    flag: true,
                    ..TextCaret::new(row.to_display(remote.caret), remote.color, CARET_WIDTH)
                });
            }
        }
        if let Some(byte) = cx.drop_caret.get().filter(|byte| contains(*byte)) {
            carets.push(TextCaret::new(row.to_display(byte), colors.caret, CARET_WIDTH));
        }
        if focused {
            for byte in cx.state.caret_indices() {
                if contains(byte) {
                    carets.push(TextCaret {
                        blink: true,
                        ..TextCaret::new(row.to_display(byte), colors.caret, CARET_WIDTH)
                    });
                }
            }
        }
        carets
    }));
    let anchor_at = create_memo(clone!(cx model -> move || {
        let row = model.get();
        cx.state.cursors().get();
        cx.focused.get();
        let byte = cx.state.caret_indices().first().copied()?;
        (byte >= row.start && byte <= row.end).then(|| row.to_display(byte))
    }));
    let anchored = create_memo(clone!(anchor_at -> move || {
        layer != Layer::Handles && anchor_at.get().is_some()
    }));
    let color = create_memo(clone!(cx -> move || cx.colors.get().syntax.markdown_plain_text));
    let font_size = create_memo(clone!(cx -> move || cx.font_size.get()));
    let item_cx = cx.clone();
    let item_model = model.clone();
    let anchor = cx.anchor;
    let anchor_position = create_memo(clone!(anchor_at -> move || anchor_at.get()));
    let anchor_node = create_memo(clone!(anchored -> move || anchored.get().then_some(anchor)));
    view! {
        <Text
            @node_ref=&node
            string={display}
            spans={spans}
            wrap
            line_padding={padding}
            font_size={font_size}
            color={color}
            marks={marks}
            carets={carets}
        >
            <ForEach keys={inline}>
                {move |index: usize| {
                    let cx = item_cx.clone();
                    let model = item_model.clone();
                    view! {
                        <TextItem>
                            <InlineView cx model index />
                        </TextItem>
                    }
                }}
            </ForEach>
            <Show condition={anchored}>
                {move || view! {
                    <TextItem at={anchor_position.clone()}>
                        <Portal node={anchor_node.clone()} />
                    </TextItem>
                }}
            </Show>
        </Text>
    }
}

#[component]
fn InlineView(cx: Context, model: Memo<Rc<Row>>, index: usize) -> NodeId {
    let item = create_memo(clone!(model -> move || model.get().inline.get(index).cloned()));
    let checkbox = create_memo(
        clone!(item -> move || matches!(item.get().map(|item| item.inline), Some(Inline::Checkbox { .. }))),
    );
    let widget = create_memo(clone!(checkbox -> move || !checkbox.get()));
    let (checkbox_cx, widget_cx) = (cx.clone(), cx);
    let (checkbox_item, widget_item) = (item.clone(), item);
    view! {
        <List spacing=0.0>
            <Show condition={checkbox}>
                {move || view! {
                    <CheckboxBox cx={checkbox_cx.clone()} item={checkbox_item.clone()} />
                }}
            </Show>
            <Show condition={widget}>
                {move || view! {
                    <WidgetPill cx={widget_cx.clone()} item={widget_item.clone()} />
                }}
            </Show>
        </List>
    }
}

#[component]
fn CheckboxBox(cx: Context, item: Memo<Option<InlineItem>>) -> NodeId {
    let checked = create_memo(
        clone!(item -> move || matches!(item.get().map(|item| item.inline), Some(Inline::Checkbox { checked: true, .. }))),
    );
    let fill = create_memo(clone!(cx checked -> move || match checked.get() {
        true => cx.colors.get().widget,
        false => Color32::TRANSPARENT,
    }));
    let outline = create_memo(clone!(cx -> move || cx.colors.get().gutter_arrow));
    let size = CHECKBOX_WIDTH;
    view! {
        <Frame
            width=size
            height=size
            color={fill}
            outline={outline}
            outline_width=CHECKBOX_OUTLINE
            outline_visible=true
            radius=CHECKBOX_RADIUS
        >
            <List spacing=0.0>
                <Show condition={checked}>
                    {move || view! {
                        <Text
                            @sizing=ItemSize::Percent(100.0)
                            string={ICON_CHECK.to_owned()}
                            icon=true
                            font_size=14.0
                            color={Color32::WHITE}
                            align=TextAlign::Center
                        />
                    }}
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn WidgetPill(cx: Context, item: Memo<Option<InlineItem>>) -> NodeId {
    let index = create_memo(
        clone!(item -> move || match item.get().map(|item| item.inline) {
            Some(Inline::Widget(index)) => Some(index),
            _ => None,
        }),
    );
    let widget = create_memo(clone!(cx index -> move || {
        let index = index.get()?;
        cx.widgets.get().get(index).cloned()
    }));
    let fill = create_memo(clone!(cx widget -> move || {
        let colors = cx.colors.get();
        match widget.get().is_some_and(|widget| widget.broken) {
            true => colors.broken_widget,
            false => colors.widget,
        }
    }));
    let width =
        create_memo(clone!(item -> move || Some(item.get().map_or(0.0, |item| item.size.x))));
    let icon = create_memo(
        clone!(widget -> move || widget.get().and_then(|widget| widget.icon).unwrap_or("").to_owned()),
    );
    let has_icon = create_memo(clone!(icon -> move || !icon.get().is_empty()));
    let label =
        create_memo(clone!(item -> move || item.get().map(|item| item.label).unwrap_or_default()));
    let color = create_memo(
        clone!(item -> move || item.get().map_or(Color32::WHITE, |item| item.style.color)),
    );
    let font_size = create_memo(
        clone!(item -> move || item.get().map_or(BODY_SIZE, |item| item.style.font.size)),
    );
    let bold =
        create_memo(clone!(item -> move || item.get().is_some_and(|item| item.style.font.bold)));
    let italic =
        create_memo(clone!(item -> move || item.get().is_some_and(|item| item.style.font.italic)));
    let monospace = create_memo(
        clone!(item -> move || item.get().is_some_and(|item| item.style.font.family == beui_core::font::FontFamily::Monospace)),
    );
    let selected = create_memo(clone!(cx index -> move || {
        cx.state.cursors().get();
        let Some(index) = index.get() else {
            return false;
        };
        let Some(widget) = cx.widgets.get().get(index).cloned() else {
            return false;
        };
        let ranges = cx.state.selection_ranges();
        cx.selected_widget.is_some() && ranges.len() == 1 && ranges[0] == widget.range && !widget.block()
    }));
    let pill = NodeRef::new();
    let overlay_anchor = pill.clone();
    let popup_cx = cx.clone();
    let popup_index = index.clone();
    view! {
        <Frame
            @node_ref=&pill
            width={width}
            height=INLINE_WIDGET_HEIGHT
            color={fill}
            radius=INLINE_WIDGET_RADIUS
        >
            <List direction=beui_core::base::Direction::Horizontal spacing=0.0>
                <Show condition={has_icon}>
                    {move || view! {
                        <Frame width={INLINE_WIDGET_ICON_INSET * 2.0}>
                            <Text
                                string={icon.clone()}
                                icon=true
                                font_size=16.0
                                color={Color32::WHITE}
                                align=TextAlign::Center
                            />
                        </Frame>
                    }}
                </Show>
                <Text
                    string={label}
                    font_size={font_size}
                    color={color}
                    bold={bold}
                    italic={italic}
                    monospace={monospace}
                    vertical_align=TextAlign::Center
                />
                <Show condition={selected}>
                    {move || {
                        let anchor = overlay_anchor.clone();
                        let render = popup_cx
                            .selected_widget
                            .clone()
                            .expect("a widget only offers a popup when the area was given one");
                        let index = popup_index.get_untracked().unwrap_or_default();
                        view! {
                            <WidgetPopup anchor render index />
                        }
                    }}
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn WidgetPopup(anchor: NodeRef, render: RenderFn<usize>, index: usize) -> NodeId {
    let content = render.call(index);
    view! {
        <Overlay
            anchor={OverlayAnchor::Node(anchor)}
            placement=Placement::BelowStart
            mode=OverlayMode::Floating
            traps_focus=false
            open=true
        >
            {content}
        </Overlay>
    }
}

#[component]
fn BlockSlot(render: RenderFn<usize>, index: usize, size: Vec2) -> NodeId {
    let content = render.call(index);
    view! {
        <Frame width={size.x} height={size.y}>{content}</Frame>
    }
}

#[component]
fn Gutter(cx: Context, model: Memo<Rc<Row>>) -> NodeId {
    let number = create_memo(clone!(model -> move || (model.get().line + 1).to_string()));
    let height = create_memo(clone!(model -> move || Some(model.get().line_height)));
    let arrow = create_memo(clone!(cx model -> move || {
        cx.state.content().get();
        let start = model.get().start;
        cx.state
            .sections()
            .iter()
            .find(|section| section.line_start == start)
            .map(|section| match section.collapsed || section.revealed {
                true => ICON_KEYBOARD_ARROW_RIGHT,
                false => ICON_KEYBOARD_ARROW_DOWN,
            })
            .unwrap_or("")
            .to_owned()
    }));
    let arrow_color = create_memo(clone!(cx -> move || cx.colors.get().gutter_arrow));
    let number_color = create_memo(clone!(cx -> move || cx.colors.get().gutter_text));
    view! {
        <Frame padding_left=GUTTER_PADDING_LEFT padding_right=GUTTER_PADDING_RIGHT>
            <List direction=beui_core::base::Direction::Horizontal spacing=0.0>
                <Frame width=GUTTER_ARROW_SIZE height={height.clone()}>
                    <Text
                        string={arrow}
                        icon=true
                        font_size=GUTTER_ARROW_SIZE
                        color={arrow_color}
                        align=TextAlign::Center
                    />
                </Frame>
                <Frame @sizing=ItemSize::Percent(100.0) height={height}>
                    <Text
                        string={number}
                        monospace=true
                        font_size=GUTTER_TEXT_SIZE
                        color={number_color}
                        align=TextAlign::End
                    />
                </Frame>
            </List>
        </Frame>
    }
}
