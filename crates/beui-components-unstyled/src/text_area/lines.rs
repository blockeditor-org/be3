use beui_core::layer::LayerPolicy;
use std::collections::HashMap;
use std::rc::Rc;

use beui_macros::{component, view};

use beui_core::base::overlay::{OverlayAnchor, Placement};
use beui_core::base::text::TextNode;
use beui_core::base::{Align, ItemSize};
use beui_core::color::Color32;
use beui_core::font::{FontFamily, FontId, TextAlign};
use beui_core::geometry::Vec2;
use beui_core::icons::{ICON_CHECK, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT};
use beui_core::node::NodeId;
use beui_core::rich::{TextCaret, TextMark};
use beui_view::components::overlay::Overlay;
use text_editor_core::{EditorCommand, MarkdownCommand, SynHlColorScope};

use crate::{Toggle, ToggleHandle};
use beui_view::primitives::*;
use beui_view::reactive::*;

use super::colors::TextAreaColors;
use super::rows::{
    BODY_SIZE, CHECKBOX_WIDTH, DOCUMENT_PADDING, INLINE_WIDGET_HEIGHT, INLINE_WIDGET_ICON_INSET,
    Inline, InlineItem, LINE_PADDING, RUN_CHIP_HEIGHT, RUN_CHIP_SIZE, Row, RowInputs, RowKey,
    RowOptions, SpanKind, build_row, galley, line_range, rich_layout, row_key,
};
use super::{
    CARET_WIDTH, CHECKBOX_OUTLINE, CHECKBOX_RADIUS, CODE_OUTSET, CODE_RADIUS, Context,
    GUTTER_ARROW_SIZE, GUTTER_PADDING_LEFT, GUTTER_PADDING_RIGHT, GeometryCell,
    INLINE_WIDGET_RADIUS, REMOTE_SELECTION_ALPHA, RUN_CHIP_RADIUS, RowEntry, TextCheckbox,
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
    let fill = create_memo(move || Some(cx.view_height.get()));
    let list = cx.list;
    let keys = cx.visible;
    let top = create_memo(move || Some(cx.padding.get().y));
    let bottom = create_memo(move || match cx.prose.get() {
        Some(_) => Some(cx.padding.get().y),
        None => Some((DOCUMENT_PADDING.y - cx.padding.get().y).max(0.0)),
    });
    let align = create_memo(move || match cx.prose.get() {
        Some(prose) => Align2::new(
            Align::Stretch,
            match prose.vertical_align {
                TextAlign::Start => Align::Start,
                TextAlign::Center => Align::Center,
                TextAlign::End => Align::End,
            },
        ),
        None => Align2::STRETCH,
    });
    let estimate = cx.row_estimate();
    view! {
        <Frame
            @node_ref=&canvas
            height={create_memo(move || Extent { min: fill.get(), ..Extent::INTRINSIC })}
            align={align}
        >
            <List spacing=0.0>
                <Frame height=top />
                <VirtualList @node_ref=&list keys item_size=estimate>|line: ReadSignal<usize>|
                    <AreaRow cx={cx} line />
                </VirtualList>
                <Frame height=bottom />
            </List>
        </Frame>
    }
}

#[component]
pub(super) fn SingleLine(cx: Context) -> NodeId {
    let canvas = cx.state.canvas();
    let model = row_model(cx, || 0);
    let text = create_memo(move || {
        let row = model.get();
        cx.metrics.get();
        let size = cx.font_size.get();
        let body =
            galley("", FontId::proportional(size)).map_or(0.0, |galley| galley.line_height());
        let laid = rich_layout(&row, FontId::proportional(size), (0.0, 0.0), f32::INFINITY)
            .map_or(Vec2::ZERO, |layout| layout.size);
        Vec2::new(laid.x, laid.y.max(body))
    });
    create_effect(move || cx.text_extent.set(text.get()));
    let canvas_width = create_memo(move || text.get().x + cx.padding.get().x * 2.0 + CARET_WIDTH);
    let canvas_height = create_memo(move || text.get().y + cx.padding.get().y * 2.0);
    let x = create_memo(move || cx.padding.get().x - cx.shift.get());
    let y = create_memo(move || {
        let padding = cx.padding.get();
        let line = text.get().y;
        let height = cx.view_height.get();
        match height > line + padding.y * 2.0 {
            true => (height - line) / 2.0,
            false => padding.y,
        }
    });
    let text_width = create_memo(move || text.get().x + CARET_WIDTH);
    let text_height = create_memo(move || text.get().y);
    let text_model = model;
    view! {
        <Canvas @node_ref=&canvas width={canvas_width} height={canvas_height}>
            <CanvasItem x={x} y={y} width={text_width} height={text_height}>
                <AreaText cx={cx} model={text_model} />
            </CanvasItem>
            <CanvasItem x y width={text_width} height={text_height} clip=false>
                <RowText cx model text={GeometryCell::default()} wrap=false layer=Layer::Handles />
            </CanvasItem>
        </Canvas>
    }
}

#[component]
fn AreaRow(cx: Context, line: ReadSignal<usize>) -> NodeId {
    let model = row_model(cx, move || line.get());
    let gutter = create_memo(move || Some(cx.gutter.get()));
    let numbered = create_memo(move || cx.gutter.get() > 0.0);
    let padding = create_memo(move || (cx.padding.get().x - CODE_OUTSET.x).max(0.0));
    let inset = create_memo(move || cx.padding.get().x.min(CODE_OUTSET.x));
    let code = create_memo(move || match model.get().code_block {
        true => cx.colors.get().code_background,
        false => Color32::TRANSPARENT,
    });
    let block = create_memo(move || model.get().block);
    let renders = cx.block.is_some();
    let shown_block = create_memo(move || block.get().filter(|_| renders).map(|(index, _)| index));
    let block_ref = NodeRef::new();
    let block_node = block_ref;
    let row = NodeRef::new();
    let text = GeometryCell::default();
    register_row(&cx, move || line.get(), &row, &text, &block_ref, model);
    let gutter_model = model;
    view! {
        <Frame @node_ref=&row>
            <List direction=beui_core::base::Direction::Horizontal spacing=0.0>
                <Show condition={numbered}>
                    <Frame width=gutter>
                        <Gutter cx={cx} model={gutter_model} />
                    </Frame>
                </Show>
                <Frame
                    @sizing=ItemSize::Percent(100.0)
                    padding={create_memo(move || Sides::horizontal(padding.get()))}
                >
                    <Frame fill=code padding={create_memo(move || Sides::horizontal(inset.get()))}>
                        <List spacing=0.0>
                            <RowText cx model text wrap=true />
                            <Dynamic value={shown_block}>|index: Option<usize>| {
                                match (index, cx.block) {
                                    (Some(index), Some(render)) => {
                                        let size = block.get_untracked().unwrap_or_default().1;
                                        let node = block_node;
                                        view! {
                                            <BlockSlot @node_ref=&node render index size />
                                        }
                                    }
                                    _ => view! {
                                        <Frame />
                                    },
                                }
                            }</Dynamic>
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
    register_row(&cx, || 0, &row, &text, &NodeRef::new(), model);
    view! {
        <Frame @node_ref=&row>
            <RowText cx model text wrap=false layer=Layer::Text />
        </Frame>
    }
}

fn register_row(
    cx: &Context,
    line: impl Fn() -> usize + 'static,
    row: &NodeRef,
    geometry: &GeometryCell,
    block: &NodeRef,
    model: Memo<Rc<Row>>,
) {
    let held = cx.held;
    let (row, geometry, block) = (*row, geometry.clone(), *block);
    create_binding(move || {
        let line = line();
        let id = held.with(|held| {
            let id = held.registered.get() + 1;
            held.registered.set(id);
            held.rows.borrow_mut().insert(
                line,
                RowEntry {
                    id,
                    row,
                    geometry: geometry.clone(),
                    model,
                    block,
                },
            );
            id
        });
        on_cleanup(move || {
            held.try_with(|held| {
                let mut rows = held.rows.borrow_mut();
                if rows.get(&line).is_some_and(|entry| entry.id == id) {
                    rows.remove(&line);
                }
            });
        });
    });
}

#[derive(Clone, PartialEq, Default)]
struct RowCursors {
    selection: Vec<std::ops::Range<usize>>,
    carets: Vec<usize>,
    anchor: Option<usize>,
    handles: Vec<(beui_core::rich::CaretHandle, usize)>,
}

impl RowCursors {
    fn selects(&self, byte: usize) -> bool {
        self.selection.iter().any(|range| range.contains(&byte))
    }
}

fn row_cursors(cx: Context, model: Memo<Rc<Row>>, layer: Layer) -> Memo<RowCursors> {
    let span = create_memo(move || {
        let row = model.get();
        (row.start, row.end)
    });
    create_memo(move || {
        cx.state.cursors().get();
        let (start, end) = span.get();
        let contains = |byte: &usize| *byte >= start && *byte <= end;
        let carets = cx.state.caret_indices();
        let touch = cx.state.touch_mode().get() && layer != Layer::Text;
        let handles = match touch {
            true => {
                cx.state.caret_handle().get();
                cx.shift.get();
                untrack(|| cx.handles())
                    .into_iter()
                    .filter(|(_, byte)| contains(byte))
                    .collect()
            }
            false => Vec::new(),
        };
        RowCursors {
            selection: cx
                .state
                .selection_ranges()
                .into_iter()
                .filter(|range| range.start < range.end && range.start <= end && range.end > start)
                .map(|range| range.start.max(start)..range.end.min(end + 1))
                .collect(),
            anchor: carets.first().copied().filter(contains),
            carets: carets.into_iter().filter(contains).collect(),
            handles,
        }
    })
}

#[derive(Clone, PartialEq)]
struct RowEnv {
    colors: TextAreaColors,
    metrics: (f32, u64),
    options: RowOptions,
    client_colors: Vec<(u64, Color32)>,
}

const ROW_CACHE_LIMIT: usize = 1024;

#[derive(Default)]
pub(super) struct RowCache {
    env: Option<RowEnv>,
    rows: HashMap<u64, Vec<(RowKey, Rc<Row>)>>,
    count: usize,
}

impl RowCache {
    fn find(&mut self, env: &RowEnv, key: &RowKey) -> Option<Rc<Row>> {
        if self.env.as_ref() != Some(env) {
            self.env = Some(env.clone());
            self.rows.clear();
            self.count = 0;
            return None;
        }
        self.rows
            .get(&bytes_hash(key.bytes()))?
            .iter()
            .find(|(held, _)| held == key)
            .map(|(_, row)| Rc::clone(row))
    }

    fn remember(&mut self, key: RowKey, row: Rc<Row>) {
        if self.count >= ROW_CACHE_LIMIT {
            self.rows.clear();
            self.count = 0;
        }
        self.count += 1;
        self.rows
            .entry(bytes_hash(key.bytes()))
            .or_default()
            .push((key, row));
    }
}

fn bytes_hash(bytes: &[u8]) -> u64 {
    use std::hash::BuildHasher;

    foldhash::fast::FixedState::with_seed(0).hash_one(bytes)
}

fn row_model(cx: Context, line: impl Fn() -> usize + 'static) -> Memo<Rc<Row>> {
    let content = cx.state.content();
    create_memo(move || {
        content.get();
        let line = line();
        let starts = cx.starts.get();
        let tables = cx.tables.get();
        let widgets = cx.widgets.get();
        let composition = cx.composition.get();
        let placeholder = cx.placeholder.get();
        let env = RowEnv {
            colors: cx.colors.get(),
            metrics: cx.metrics.get(),
            options: RowOptions {
                body_size: cx.font_size.get(),
                mask: cx.masked.get(),
                single_line: cx.single_line,
                prose: cx.prose.get(),
            },
            client_colors: cx.client_colors.get(),
        };
        cx.state.with_snapshot(|snapshot| {
            let (start, end, newline) =
                line_range(&snapshot.bytes, &starts, line).unwrap_or((0, 0, false));
            let spacers = tables.get(&start).map_or(&[][..], Vec::as_slice);
            let placeholder = (snapshot.bytes.is_empty() && !placeholder.is_empty() && line == 0)
                .then_some(placeholder.as_str());
            if !snapshot.loaded || snapshot.highlight.is_none() {
                return Rc::new(Row {
                    line,
                    start,
                    end,
                    ..Row::default()
                });
            }
            let inputs = RowInputs {
                snapshot,
                widgets: &widgets,
                client_colors: &env.client_colors,
                composition: composition.as_ref(),
                colors: &env.colors,
                options: env.options,
                spacers,
                placeholder,
            };
            let key = row_key(&inputs, start, end, newline);
            let held = cx.held.value();
            if let Some(key) = &key
                && let Some(row) = held.row_cache.borrow_mut().find(&env, key)
            {
                return match row.line == line && row.start == start {
                    true => row,
                    false => Rc::new(row.moved(line, start)),
                };
            }
            cx.state.count_row_built();
            let row = Rc::new(build_row(&inputs, line, start, end, newline));
            if let Some(key) = key {
                held.row_cache.borrow_mut().remember(key, Rc::clone(&row));
            }
            row
        })
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
    let placed = component_placed();
    create_effect(move || {
        placed.get();
        if layer != Layer::Handles
            && text.borrow().is_none()
            && let Some(id) = node.try_get()
        {
            *text.borrow_mut() = with_document(|document| {
                let text = document.arena.kind_of::<TextNode>(id)?;
                Some(document.text_geometry(text))
            });
        }
    });
    let keys = create_memo(move || piece_keys(&model.get(), layer));
    let cursors = row_cursors(cx, model, layer);
    let padding = create_memo(move || match (cx.single_line, cx.prose.get()) {
        (true, _) => (0.0, 0.0),
        (false, Some(prose)) => prose.line_padding,
        (false, None) => LINE_PADDING,
    });
    let wraps = create_memo(move || wrap && cx.prose.get().is_none_or(|prose| prose.wrap));
    let align = create_memo(move || cx.prose.get().map(|prose| prose.align));
    let marks = create_memo(move || {
        if layer == Layer::Handles {
            return Vec::new();
        }
        let row = model.get();
        let colors = cx.colors.get();
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
        for range in &cursors.get().selection {
            marks.push(TextMark::new(row.display_range(range), colors.selection));
        }
        for remote in cx.remote_cursors.get() {
            let range = remote.selection;
            if range.start < range.end && range.start <= row.end && range.end > row.start {
                let [red, green, blue, _] = remote.color.to_array();
                let fill =
                    Color32::from_rgba_unmultiplied(red, green, blue, REMOTE_SELECTION_ALPHA);
                marks.push(TextMark::new(row.display_range(&range), fill));
            }
        }
        marks
    });
    let carets = create_memo(move || {
        let row = model.get();
        let colors = cx.colors.get();
        let typing = cx.focused.get() && cx.keyboard.get();
        let contains = |byte: usize| byte >= row.start && byte <= row.end;
        let mut carets = Vec::new();
        let cursors = cursors.get();
        for (handle, byte) in &cursors.handles {
            carets.push(TextCaret {
                handle: Some(*handle),
                ..TextCaret::new(row.to_display(*byte), colors.caret, CARET_WIDTH)
            });
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
            carets.push(TextCaret::new(
                row.to_display(byte),
                colors.caret,
                CARET_WIDTH,
            ));
        }
        if typing {
            for byte in &cursors.carets {
                carets.push(TextCaret {
                    blink: true,
                    ..TextCaret::new(row.to_display(*byte), colors.caret, CARET_WIDTH)
                });
            }
        }
        carets
    });
    let anchor_at = create_memo(move || {
        let row = model.get();
        cx.focused.get();
        cursors.get().anchor.map(|byte| row.to_display(byte))
    });
    let anchored = create_memo(move || layer != Layer::Handles && anchor_at.get().is_some());
    let color = create_memo(move || cx.colors.get().syntax.markdown_plain_text);
    let font = create_memo(move || {
        FontId::proportional(cx.font_size.get())
            .bold(cx.prose.get().is_some_and(|prose| prose.bold))
    });
    let item_model = model;
    let anchor = cx.anchor;
    let anchor_position = create_memo(move || anchor_at.get());
    let anchor_node = create_memo(move || anchored.get().then_some(anchor));
    view! {
        <Text
            @node_ref=&node
            wrap={wraps}
            align={align}
            vertical_align={Some(TextAlign::Start)}
            line_padding={padding}
            font={font}
            color={color}
            marks={marks}
            carets={carets}
        >
            <ForEach keys={keys}>|(key, _): (PieceKey, usize)|
                <RowPiece cx model={item_model} cursors layer key />
            </ForEach>
            <Show condition={anchored}>
                <TextItem at={anchor_position}>
                    <Portal node={anchor_node} />
                </TextItem>
            </Show>
        </Text>
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum PieceKey {
    Text {
        text: String,
        font: (u32, FontFamily, bool, bool),
        color: Color32,
        underline: bool,
        strikethrough: bool,
        break_after: bool,
        invisible: Option<usize>,
    },
    Space(u32),
    Inline(usize),
}

fn piece_keys(row: &Row, layer: Layer) -> Vec<(PieceKey, usize)> {
    let mut seen: HashMap<PieceKey, usize> = HashMap::new();
    row.spans
        .iter()
        .map(|span| {
            let key = match span.kind {
                SpanKind::Text => {
                    let style = span.style;
                    let handles = layer == Layer::Handles;
                    PieceKey::Text {
                        text: row
                            .display
                            .get(span.range.clone())
                            .unwrap_or_default()
                            .to_owned(),
                        font: (
                            style.font.size.to_bits(),
                            style.font.family,
                            style.font.bold,
                            style.font.italic,
                        ),
                        color: match handles {
                            true => Color32::TRANSPARENT,
                            false => style.color,
                        },
                        underline: style.underline && !handles,
                        strikethrough: style.strikethrough && !handles,
                        break_after: span.break_after,
                        invisible: span.invisible.filter(|_| !handles),
                    }
                }
                SpanKind::Space(width) => PieceKey::Space(width.to_bits()),
                SpanKind::Inline(index) => PieceKey::Inline(index),
            };
            let repeat = seen.entry(key.clone()).or_default();
            *repeat += 1;
            (key, *repeat - 1)
        })
        .collect()
}

#[component]
fn RowPiece(
    cx: Context,
    model: Memo<Rc<Row>>,
    cursors: Memo<RowCursors>,
    layer: Layer,
    key: PieceKey,
) -> TextChild {
    match key {
        PieceKey::Text {
            text,
            font: (size, family, bold, italic),
            color,
            underline,
            strikethrough,
            break_after,
            invisible,
        } => {
            let font = FontId::new(f32::from_bits(size), family)
                .bold(bold)
                .italic(italic);
            let color = create_memo(move || match invisible {
                Some(byte) if cursors.get().selects(byte) => {
                    cx.colors.get().syntax.scope(SynHlColorScope::Invisible)
                }
                _ => color,
            });
            view! {
                <Span text font color underline strikethrough break_after />
            }
            .into_child()
        }
        PieceKey::Space(width) => view! {
            <Gap width={f32::from_bits(width)} />
        }
        .into_child(),
        PieceKey::Inline(item) if layer == Layer::Handles => {
            let text = inline_text(model, item);
            let size = create_memo(move || {
                model
                    .get()
                    .inline
                    .get(item)
                    .map_or(Vec2::ZERO, |item| item.size)
            });
            let width = create_memo(move || Some(size.get().x));
            let height = create_memo(move || Some(size.get().y));
            view! {
                <TextItem text>
                    <Frame width=width height=height />
                </TextItem>
            }
            .into_child()
        }
        PieceKey::Inline(item) => {
            let text = inline_text(model, item);
            view! {
                <TextItem text>
                    <InlineView cx model index=item />
                </TextItem>
            }
            .into_child()
        }
    }
}

fn inline_text(model: Memo<Rc<Row>>, item: usize) -> Memo<Option<Rc<str>>> {
    create_memo(move || {
        let row = model.get();
        row.spans
            .iter()
            .find(|span| span.kind == SpanKind::Inline(item))
            .and_then(|span| row.display.get(span.range.clone()))
            .map(Rc::from)
    })
}

#[component]
fn InlineView(cx: Context, model: Memo<Rc<Row>>, index: usize) -> NodeId {
    let item = create_memo(move || model.get().inline.get(index).cloned());
    let checkbox = create_memo(move || {
        matches!(
            item.get().map(|item| item.inline),
            Some(Inline::Checkbox { .. })
        )
    });
    let run = create_memo(move || matches!(item.get().map(|item| item.inline), Some(Inline::Run)));
    let widget = create_memo(move || !checkbox.get() && !run.get());
    let (checkbox_item, widget_item, run_item) = (item, item, item);
    view! {
        <List spacing=0.0>
            <Show condition={checkbox}>
                <InlineCheckbox cx={cx} item={checkbox_item} />
            </Show>
            <Show condition={widget}>
                <WidgetPill cx={cx} item={widget_item} />
            </Show>
            <Show condition={run}>
                <RunChip cx={cx} item={run_item} />
            </Show>
        </List>
    }
}

#[component]
fn RunChip(cx: Context, item: Memo<Option<InlineItem>>) -> NodeId {
    let fill = create_memo(move || cx.colors.get().widget);
    let width = create_memo(move || Some(item.get().map_or(0.0, |item| item.size.x)));
    let label = create_memo(move || item.get().map(|item| item.label).unwrap_or_default());
    let color = create_memo(move || item.get().map_or(Color32::WHITE, |item| item.style.color));
    view! {
        <Frame width=width height=RUN_CHIP_HEIGHT fill=fill radius=RUN_CHIP_RADIUS>
            <Text
                string={label}
                font={FontId::monospace(RUN_CHIP_SIZE)}
                color={color}
                align=TextAlign::Center
                vertical_align=TextAlign::Center
            />
        </Frame>
    }
}

#[component]
fn InlineCheckbox(cx: Context, item: Memo<Option<InlineItem>>) -> NodeId {
    let checked = create_memo(move || {
        matches!(
            item.get().map(|item| item.inline),
            Some(Inline::Checkbox { checked: true, .. })
        )
    });
    let on_change = Callback::new(move |_: bool| {
        let Some(Inline::Checkbox { line_start, .. }) =
            item.get_untracked().map(|item| item.inline)
        else {
            return;
        };
        let position = cx.state.with_core(|core| core.position(line_start));
        cx.state
            .execute(EditorCommand::Markdown(MarkdownCommand::ToggleCheckbox(
                position,
            )));
    });
    let disabled = cx.disabled;
    if let Some(render) = &cx.checkbox {
        return render.call(TextCheckbox {
            checked,
            disabled,
            on_change,
        });
    }
    view! {
        <Toggle
            checked
            disabled
            capture_presses=true
            tab_stop=false
            press_focus=false
            on_change={on_change}
        >|handle: ToggleHandle|
            <CheckboxBox cx handle />
        </Toggle>
    }
}

#[component]
fn CheckboxBox(cx: Context, handle: ToggleHandle) -> NodeId {
    let checked = handle.checked;
    let fill = create_memo(move || match checked.get() {
        true => cx.colors.get().widget,
        false => Color32::TRANSPARENT,
    });
    let outline = create_memo(move || cx.colors.get().gutter_arrow);
    let size = CHECKBOX_WIDTH;
    view! {
        <Frame
            width=size
            height=size
            fill=fill
            outline={create_memo(move || Some(Outline::new(outline.get(), CHECKBOX_OUTLINE)))}
            radius=CHECKBOX_RADIUS
        >
            <List spacing=0.0>
                <Show condition={checked}>
                    <Text
                        @sizing=ItemSize::Percent(100.0)
                        color={Color32::WHITE}
                        align=TextAlign::Center
                    >
                        <Icon glyph={ICON_CHECK.to_owned()} />
                    </Text>
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn WidgetPill(cx: Context, item: Memo<Option<InlineItem>>) -> NodeId {
    let index = create_memo(move || match item.get().map(|item| item.inline) {
        Some(Inline::Widget(index)) => Some(index),
        _ => None,
    });
    let widget = create_memo(move || {
        let index = index.get()?;
        cx.widgets.get().get(index).cloned()
    });
    let fill = create_memo(move || {
        let colors = cx.colors.get();
        match widget.get().is_some_and(|widget| widget.broken) {
            true => colors.broken_widget,
            false => colors.widget,
        }
    });
    let width = create_memo(move || Some(item.get().map_or(0.0, |item| item.size.x)));
    let icon = create_memo(move || {
        widget
            .get()
            .and_then(|widget| widget.icon)
            .unwrap_or("")
            .to_owned()
    });
    let has_icon = create_memo(move || !icon.get().is_empty());
    let label = create_memo(move || item.get().map(|item| item.label).unwrap_or_default());
    let color = create_memo(move || item.get().map_or(Color32::WHITE, |item| item.style.color));
    let font = create_memo(move || {
        item.get()
            .map_or(FontId::proportional(BODY_SIZE), |item| item.style.font)
    });
    let selected = create_memo(move || {
        cx.state.cursors().get();
        let Some(index) = index.get() else {
            return false;
        };
        let Some(widget) = cx.widgets.get().get(index).cloned() else {
            return false;
        };
        let ranges = cx.state.selection_ranges();
        cx.selected_widget.is_some()
            && ranges.len() == 1
            && ranges[0] == widget.range
            && !widget.block()
    });
    let pill = NodeRef::new();
    let overlay_anchor = pill;
    let popup_index = index;
    view! {
        <Frame
            @node_ref=&pill
            width=width
            height=INLINE_WIDGET_HEIGHT
            fill=fill
            radius=INLINE_WIDGET_RADIUS
        >
            <List direction=beui_core::base::Direction::Horizontal spacing=0.0>
                <Show condition={has_icon}>|| {
                    let icon = icon;
                    view! {
                        <Frame width={INLINE_WIDGET_ICON_INSET * 2.0}>
                            <Text
                                font={FontId::proportional(16.0)}
                                color={Color32::WHITE}
                                align=TextAlign::Center
                            >
                                <Icon glyph={icon} />
                            </Text>
                        </Frame>
                    }
                }</Show>
                <Text string={label} font={font} color={color} vertical_align=TextAlign::Center />
                <Show condition={selected}>|| {
                    let anchor = overlay_anchor;
                    let render = cx
                        .selected_widget
                        .expect("a widget only offers a popup when the area was given one");
                    let index = popup_index.get_untracked().unwrap_or_default();
                    view! {
                        <WidgetPopup anchor render index />
                    }
                }</Show>
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
            policy=LayerPolicy::Pass
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
    let number = create_memo(move || (model.get().line + 1).to_string());
    let height = create_memo(move || Some(model.get().line_height));
    let arrow = create_memo(move || {
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
    });
    let arrow_color = create_memo(move || cx.colors.get().gutter_arrow);
    let number_color = create_memo(move || cx.colors.get().gutter_text);
    view! {
        <Frame padding={Sides::new(GUTTER_PADDING_LEFT, 0.0, GUTTER_PADDING_RIGHT, 0.0)}>
            <List direction=beui_core::base::Direction::Horizontal spacing=0.0>
                <Frame width=GUTTER_ARROW_SIZE height=height>
                    <Text
                        font={FontId::proportional(GUTTER_ARROW_SIZE)}
                        color={arrow_color}
                        align=TextAlign::Center
                    >
                        <Icon glyph={arrow} />
                    </Text>
                </Frame>
                <Frame @sizing=ItemSize::Percent(100.0) height=height>
                    <Text
                        string={number}
                        font={FontId::monospace(BODY_SIZE)}
                        color={number_color}
                        align=TextAlign::End
                    />
                </Frame>
            </List>
        </Frame>
    }
}
