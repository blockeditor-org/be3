use beui_view::types::LayerPolicy;
use beui_view::types::Overflow;
use stable_hash::HashMap;
use std::rc::Rc;

use beui_macros::{component, view};

use beui_view::components::overlay::Overlay;
use beui_view::reactive::NodeId;
use beui_view::reactive::OverlayAnchor;
use beui_view::types::Color;
use beui_view::types::Placement;
use beui_view::types::icons::{ICON_CHEVRON_DOWN, ICON_CHEVRON_RIGHT};
use beui_view::types::{Align, ItemSize};
use beui_view::types::{FontFamily, FontId, TextAlign, TextLayout};
use beui_view::types::{Rect, Vec2};
use beui_view::types::{SpanStyle, TextCaret, TextGlyph, TextMark};
use text_editor_core::{SynHlColorScope, SynHlStyle, TextLanguage};

use beui_view::primitives::*;
use beui_view::reactive::*;

use super::colors::TextAreaColors;
use super::hooks::{MARGIN_MIN_WIDTH_EM, Margin, MarkerKind, TextLeaf, TextLine, TextMarker};
use super::rows::{
    Inline, InlineItem, Row, RowInputs, RowKey, RowOptions, SpanKind, TableColumns, build_row,
    line_range, row_key, table_columns,
};
use super::{Context, GeometryCell, Held, RowEntry};
use beui_view::types::{Align2, Extent, Sides};

#[component]
pub(super) fn Lines(cx: Context) -> NodeId {
    let canvas = cx.state.canvas();
    let fill = create_memo(move || Some(cx.view_height.get()));
    let list = cx.list;
    let keys = cx.visible;
    let top = create_memo(move || Some(cx.padding.get().top));
    let bottom = create_memo(move || match cx.prose.get() {
        Some(_) => Some(cx.padding.get().bottom),
        None => Some((cx.geometry.get().end_room - cx.padding.get().bottom).max(0.0)),
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
    let lines = view! {
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
    };
    cx.held.with(|held| {
        held.track(canvas.get());
        held.track(list.get());
    });
    let area = last_rect(canvas.get());
    let rows = last_rect(list.get());
    let content = create_memo(move || {
        let left = area.get().min.x;
        let padding = cx.padding.get();
        let right = cx
            .rights
            .with(|rights| rights.values().copied().reduce(f32::max))?;
        let below = bottom.get().unwrap_or(0.0);
        let scale = tracked_scale(cx.held);
        Some(Vec2::new(
            (right.max(left) - left) / scale + padding.right,
            padding.top + rows.get().height() / scale + below,
        ))
    });
    create_effect(move || {
        if let Some(size) = content.get() {
            untrack(|| cx.on_content_size.call(size));
        }
    });
    lines
}

fn last_rect(node: NodeId) -> ReadSignal<Rect> {
    let measured = measure(node);
    let held = create_signal(Rect::ZERO);
    create_effect(move || {
        if let Some(measured) = measured.get() {
            held.set(measured.rect);
        }
    });
    held.read_only()
}

pub(super) fn tracked_scale(held: Stored<Held>) -> f32 {
    held.with(|held| {
        held.rows_measured.get();
        let texts: Vec<TextGeometrySignal> = held.texts.borrow().values().copied().collect();
        texts
            .into_iter()
            .find_map(|text| text.get())
            .map_or(1.0, |text| text.scale())
    })
    .max(f32::EPSILON)
}

pub(super) fn scale_of(held: &Stored<Held>) -> f32 {
    held.with(|held| {
        held.rows
            .borrow()
            .values()
            .find_map(|entry| entry.text(held))
            .map_or(1.0, |text| text.scale())
    })
    .max(f32::EPSILON)
}

#[component]
pub(super) fn SingleLine(cx: Context) -> NodeId {
    let canvas = cx.state.canvas();
    let model = row_model(cx, || 0);
    let bounds = create_signal(Rect::ZERO);
    let geometry = GeometryCell::default();
    let text = create_memo(move || {
        let scale = tracked_scale(cx.held);
        let laid = bounds.get().size() / scale;
        cx.metrics.get();
        let body = lay_out_text(
            "",
            FontId::proportional(cx.font_size.get()),
            TextLayout::DEFAULT,
        )
        .map_or(0.0, |line| line.line_height());
        Vec2::new(laid.x, laid.y.max(body))
    });
    create_effect(move || cx.text_extent.set(text.get()));
    let canvas_width = create_memo(move || {
        let padding = cx.padding.get();
        text.get().x + padding.left + padding.right + cx.geometry.get().caret_width
    });
    let canvas_height = create_memo(move || {
        let padding = cx.padding.get();
        text.get().y + padding.top + padding.bottom
    });
    let x = create_memo(move || cx.padding.get().left - cx.shift.get());
    let y = create_memo(move || {
        let padding = cx.padding.get();
        let line = text.get().y;
        let height = cx.view_height.get();
        match height > line + padding.top + padding.bottom {
            true => (height - line) / 2.0,
            false => padding.top,
        }
    });
    let text_width = create_memo(move || text.get().x + cx.geometry.get().caret_width);
    let text_height = create_memo(move || text.get().y);
    let built = view! {
        <Canvas @node_ref=&canvas width={canvas_width} height={canvas_height} scrolls_across=true>
            <CanvasItem x={x} y={y} width={text_width} height={text_height}>
                <AreaText cx={cx} model={model} geometry bounds />
            </CanvasItem>
        </Canvas>
    };
    cx.held.with(|held| held.track(built));
    built
}

#[component]
fn AreaRow(cx: Context, line: ReadSignal<usize>) -> NodeId {
    let model = row_model(cx, move || line.get());
    let gutter = create_memo(move || Some(cx.gutter.get()));
    let numbered = create_memo(move || cx.gutter.get() > 0.0);
    let look = create_memo(move || model.get().look.clone());
    let padding = create_memo(move || cx.padding.get());
    let block = create_memo(move || model.get().block);
    let renders = cx.block.is_some();
    let shown_block = create_memo(move || block.get().filter(|_| renders).map(|(index, _)| index));
    let block_ref = NodeRef::new();
    let row = NodeRef::new();
    let text = GeometryCell::default();
    register_row(&cx, move || line.get(), &row, &text, &block_ref, model);
    let id = cx.held.with(|held| {
        let id = held.registered.get() + 1;
        held.registered.set(id);
        id
    });
    let rights = cx.rights;
    on_cleanup(move || {
        if rights.is_alive() {
            rights.update(|rights| {
                rights.remove(&id);
            });
        }
    });
    let bounds = move |bounds: Rect| {
        rights.update(|rights| {
            rights.insert(id, bounds.max.x);
        })
    };
    let lines = view! {
        <List spacing=0.0>
            <RowText cx model text wrap=true on_bounds=bounds />
            <Dynamic value={shown_block}>|index: Option<usize>| {
                match (index, cx.block) {
                    (Some(index), Some(render)) => {
                        let size = block.get_untracked().unwrap_or_default().1;
                        view! {
                            <BlockSlot @node_ref=&block_ref render index size />
                        }
                    }
                    _ => view! {
                        <Frame />
                    },
                }
            }</Dynamic>
        </List>
    };
    let content = cx.state.content();
    let margin = create_memo(move || {
        content.get();
        let document = cx.state.with_snapshot(|snapshot| {
            matches!(
                snapshot.language,
                TextLanguage::Markdown | TextLanguage::RichText
            )
        });
        if !document || !cx.line_numbers || cx.prose.get().is_some() || cx.single_line {
            return Margin::default();
        }
        let size = cx.font_size.get();
        let padding = cx.padding.get();
        let width = cx.view_width.get() - cx.gutter.get() - padding.left - padding.right;
        Margin::centered(
            width,
            size,
            cx.view_width.get() >= size * MARGIN_MIN_WIDTH_EM,
        )
    });
    let wrapped = cx.line.call(TextLine {
        state: cx.state,
        line: create_memo(move || model.get().line),
        start: create_memo(move || model.get().start),
        look,
        padding,
        size: cx.font_size,
        margin,
        outdent: create_memo(move || model.get().outdent),
        space: create_memo(move || model.get().space),
        table: create_memo(move || model.get().table.clone()),
        text: lines,
    });
    let built = view! {
        <Frame @node_ref=&row>
            <List direction=beui_view::types::Direction::Horizontal spacing=0.0>
                <Show condition={numbered}>
                    <Frame width=gutter>
                        <Gutter cx={cx} model={model} />
                    </Frame>
                </Show>
                <Frame @sizing=ItemSize::Percent(100.0)>{wrapped}</Frame>
            </List>
        </Frame>
    };
    track_row(&cx, built);
    built
}

fn track_row(cx: &Context, row: NodeId) {
    let held = cx.held;
    held.with(|held| held.track(row));
    on_cleanup(move || {
        held.try_with(|held| held.forget(row));
    });
}

#[component]
fn AreaText(
    cx: Context,
    model: Memo<Rc<Row>>,
    geometry: GeometryCell,
    bounds: Signal<Rect>,
) -> NodeId {
    let row = NodeRef::new();
    register_row(&cx, || 0, &row, &geometry, &NodeRef::new(), model);
    let built = view! {
        <Frame @node_ref=&row>
            <RowText
                cx
                model
                text={geometry}
                wrap=false
                on_bounds={move |laid: Rect| bounds.set(laid)}
            />
        </Frame>
    };
    track_row(&cx, built);
    built
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
    handles: Vec<(beui_view::types::CaretHandle, usize)>,
}

impl RowCursors {
    fn selects(&self, byte: usize) -> bool {
        self.selection.iter().any(|range| range.contains(&byte))
    }
}

fn row_cursors(cx: Context, model: Memo<Rc<Row>>) -> Memo<RowCursors> {
    let span = create_memo(move || {
        let row = model.get();
        (row.start, row.end)
    });
    create_memo(move || {
        cx.state.cursors().get();
        let (start, end) = span.get();
        let contains = |byte: &usize| *byte >= start && *byte <= end;
        let carets = cx.state.caret_indices();
        let touch = cx.state.touch_mode().get();
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
    client_colors: Vec<(u64, Color)>,
}

const ROW_CACHE_LIMIT: usize = 1024;
const TABLE_CACHE_LIMIT: usize = 64;

#[derive(Default)]
pub(super) struct RowCache {
    env: Option<RowEnv>,
    rows: HashMap<u64, Vec<(RowKey, Rc<Row>)>>,
    count: usize,
    tables: HashMap<usize, (u64, Rc<TableColumns>)>,
}

impl RowCache {
    fn refresh(&mut self, env: &RowEnv) -> bool {
        if self.env.as_ref() == Some(env) {
            return false;
        }
        self.env = Some(env.clone());
        self.rows.clear();
        self.tables.clear();
        self.count = 0;
        true
    }

    fn table(
        &mut self,
        env: &RowEnv,
        revision: u64,
        start: usize,
        measure: impl FnOnce() -> TableColumns,
    ) -> Rc<TableColumns> {
        self.refresh(env);
        if self.tables.len() >= TABLE_CACHE_LIMIT {
            self.tables.clear();
        }
        match self.tables.get(&start) {
            Some((held, columns)) if *held == revision => Rc::clone(columns),
            _ => {
                let columns = Rc::new(measure());
                self.tables.insert(start, (revision, Rc::clone(&columns)));
                columns
            }
        }
    }

    fn find(&mut self, env: &RowEnv, key: &RowKey) -> Option<Rc<Row>> {
        if self.refresh(env) {
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
                geometry: cx.geometry.get(),
            },
            client_colors: cx.client_colors.get(),
        };
        cx.state.with_snapshot(|snapshot| {
            let (start, end, newline) =
                line_range(&snapshot.bytes, &starts, line).unwrap_or((0, 0, false));
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
            let mut inputs = RowInputs {
                snapshot,
                widgets: &widgets,
                client_colors: &env.client_colors,
                composition: composition.as_ref(),
                colors: &env.colors,
                options: env.options,
                placeholder,
                columns: None,
            };
            let held = cx.held.value();
            if let Some(table) = snapshot.highlight().line_look(start).table {
                let columns =
                    held.row_cache
                        .borrow_mut()
                        .table(&env, snapshot.revision, table.start, || {
                            table_columns(&inputs, table.start)
                        });
                inputs.columns = Some(columns);
            }
            let key = row_key(&inputs, start, end, newline);
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
    on_bounds: Callback<Rect>,
) -> NodeId {
    let node = NodeRef::new();
    let keys = create_memo(move || piece_keys(&model.get()));
    let cursors = row_cursors(cx, model);
    let padding = create_memo(move || match (cx.single_line, cx.prose.get()) {
        (true, _) => (0.0, 0.0),
        (false, Some(prose)) => prose.line_padding,
        (false, None) => cx.geometry.get().line_padding,
    });
    let wraps = create_memo(
        move || match wrap && cx.prose.get().is_none_or(|prose| prose.wrap) {
            true => Overflow::default(),
            false => Overflow::Visible,
        },
    );
    let align = create_memo(move || cx.prose.get().map(|prose| prose.align));
    let hang = create_memo(move || model.get().hang);
    let marks = create_memo(move || {
        let row = model.get();
        let colors = cx.colors.get();
        let mut marks = Vec::new();
        for range in &cursors.get().selection {
            marks.push(TextMark::new(row.display_range(range), colors.selection));
        }
        for remote in cx.remote_cursors.get() {
            let range = remote.selection;
            if range.start < range.end && range.start <= row.end && range.end > row.start {
                let fill = remote
                    .color
                    .scale_alpha(f32::from(colors.remote_selection_alpha) / 255.0);
                marks.push(TextMark::new(row.display_range(&range), fill));
            }
        }
        marks
    });
    let glyphs = create_memo(move || {
        let row = model.get();
        let cursors = cursors.get();
        let color = cx.colors.get().syntax.scope(SynHlColorScope::Invisible);
        row.invisibles
            .iter()
            .filter(|invisible| invisible.trailing || cursors.selects(invisible.source))
            .map(|invisible| {
                TextGlyph::new(
                    invisible.display.clone(),
                    invisible.glyph,
                    invisible.font,
                    color,
                )
            })
            .collect::<Vec<_>>()
    });
    let carets = create_memo(move || {
        let row = model.get();
        let colors = cx.colors.get();
        let caret_width = cx.geometry.get().caret_width;
        let typing = cx.focused.get() && cx.keyboard.get();
        let contains = |byte: usize| byte >= row.start && byte <= row.end;
        let mut carets = Vec::new();
        let cursors = cursors.get();
        for (handle, byte) in &cursors.handles {
            carets.push(TextCaret {
                handle: Some(*handle),
                ..TextCaret::new(row.to_display(*byte), colors.caret, caret_width)
            });
        }
        for remote in cx.remote_cursors.get() {
            if contains(remote.caret) {
                carets.push(TextCaret {
                    flag: true,
                    ..TextCaret::new(row.to_display(remote.caret), remote.color, caret_width)
                });
            }
        }
        if let Some(byte) = cx.drop_caret.get().filter(|byte| contains(*byte)) {
            carets.push(TextCaret::new(
                row.to_display(byte),
                colors.caret,
                caret_width,
            ));
        }
        if typing {
            for byte in &cursors.carets {
                carets.push(TextCaret {
                    blink: true,
                    ..TextCaret::new(row.to_display(*byte), colors.caret, caret_width)
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
    let anchored = create_memo(move || anchor_at.get().is_some());
    let color = create_memo(move || cx.colors.get().syntax.markdown_plain_text);
    let font = create_memo(move || {
        FontId::proportional(cx.font_size.get())
            .bold(cx.prose.get().is_some_and(|prose| prose.bold))
    });
    let anchor = cx.anchor;
    let anchor_position = create_memo(move || anchor_at.get());
    let anchor_node = create_memo(move || anchored.get().then_some(anchor));
    let shown = view! {
        <Text
            @node_ref=&node
            overflow={wraps}
            align={align}
            vertical_align={Some(TextAlign::Start)}
            line_padding={padding}
            hang={hang}
            font={font}
            color={color}
            marks={marks}
            glyphs={glyphs}
            carets={carets}
        >
            <ForEach keys={keys}>|(key, _): (PieceKey, usize)|
                <RowPiece cx model={model} key />
            </ForEach>
            <Show condition={anchored}>
                <TextItem at={anchor_position}>
                    <Portal node={anchor_node} />
                </TextItem>
            </Show>
        </Text>
    };
    text.set(Some(shown));
    let held = cx.held;
    held.with(|held| held.track_text(shown));
    on_cleanup(move || {
        held.try_with(|held| held.forget(shown));
    });
    let measured = measure_text(shown);
    let bounds = create_signal(Rect::ZERO);
    create_effect(move || {
        if let Some(geometry) = measured.get() {
            bounds.set(geometry.bounds());
        }
    });
    create_effect(move || {
        let laid = bounds.get();
        untrack(|| on_bounds.call(laid));
    });
    shown
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum InlineKind {
    Marker,
    Quote,
    Widget,
    Run,
    Void,
}

impl InlineKind {
    fn of(inline: Inline) -> Self {
        match inline {
            Inline::Marker(_) => Self::Marker,
            Inline::Quote => Self::Quote,
            Inline::Widget(_) => Self::Widget,
            Inline::Run => Self::Run,
            Inline::Void => Self::Void,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum PieceKey {
    Text {
        text: String,
        font: (u32, FontFamily, bool, bool),
        color: Color,
        underline: bool,
        strikethrough: bool,
        leaf: Option<SynHlStyle>,
    },
    Space(u32),
    Inline(usize, InlineKind),
}

fn piece_keys(row: &Row) -> Vec<(PieceKey, usize)> {
    let mut seen: HashMap<PieceKey, usize> = HashMap::default();
    row.spans
        .iter()
        .map(|span| {
            let key = match span.kind {
                SpanKind::Text => {
                    let style = span.style;
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
                        color: style.color,
                        underline: style.underline,
                        strikethrough: style.strikethrough,
                        leaf: span.leaf,
                    }
                }
                SpanKind::Space(width) => PieceKey::Space(width.to_bits()),
                SpanKind::Inline(index) => PieceKey::Inline(
                    index,
                    row.inline
                        .get(index)
                        .map_or(InlineKind::Quote, |item| InlineKind::of(item.inline)),
                ),
            };
            let repeat = seen.entry(key.clone()).or_default();
            *repeat += 1;
            (key, *repeat - 1)
        })
        .collect()
}

#[component]
fn RowPiece(cx: Context, model: Memo<Rc<Row>>, key: PieceKey) -> TextChild {
    match key {
        PieceKey::Text {
            text,
            font: (size, family, bold, italic),
            color,
            underline,
            strikethrough,
            leaf,
        } => {
            let font = FontId::new(f32::from_bits(size), family)
                .bold(bold)
                .italic(italic);
            let style = SpanStyle {
                font,
                color,
                underline,
                strikethrough,
            };
            match leaf {
                Some(syntax) => cx
                    .leaf
                    .call(TextLeaf {
                        text,
                        style,
                        syntax,
                    })
                    .into_child(),
                None => view! {
                    <Span font color={Some(color)} underline strikethrough>{text}</Span>
                }
                .into_child(),
            }
        }
        PieceKey::Space(width) => view! {
            <Gap width={f32::from_bits(width)} />
        }
        .into_child(),
        PieceKey::Inline(item, kind) => {
            let text = inline_text(model, item);
            view! {
                <TextItem text>
                    <InlineView cx model index=item kind />
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
fn InlineView(cx: Context, model: Memo<Rc<Row>>, index: usize, kind: InlineKind) -> NodeId {
    let item = create_memo(move || model.get().inline.get(index).cloned());
    match kind {
        InlineKind::Marker | InlineKind::Void => cx.marker.call(marker(cx, model, item)),
        InlineKind::Quote => view! {
            <Frame />
        },
        InlineKind::Widget => view! {
            <WidgetPill cx={cx} item={item} />
        },
        InlineKind::Run => view! {
            <RunChip cx={cx} item={item} />
        },
    }
}

fn marker(cx: Context, model: Memo<Rc<Row>>, item: Memo<Option<InlineItem>>) -> TextMarker {
    let kind = create_memo(move || match item.get() {
        Some(InlineItem {
            inline: Inline::Marker(list),
            ..
        }) => MarkerKind::from(list),
        Some(item) => MarkerKind::Void(item.props),
        None => MarkerKind::Void(Default::default()),
    });
    let style = create_memo(move || {
        item.get().map_or(
            SpanStyle::new(FontId::proportional(cx.font_size.get()), Color::TRANSPARENT),
            |item| item.style,
        )
    });
    let shown = create_memo(move || kind.get() == MarkerKind::Check(true));
    let checked = Binding::new(shown, move |_: bool| {
        let start = model.get_untracked().start;
        cx.state.set_checked(start, !shown.get_untracked());
    });
    let label = create_memo(move || item.get().map(|item| item.label).unwrap_or_default());
    TextMarker {
        state: cx.state,
        line: model.get_untracked().line,
        kind,
        label,
        depth: create_memo(move || model.get().look.depth),
        style,
        checked,
        disabled: cx.locked,
    }
}

#[component]
fn RunChip(cx: Context, item: Memo<Option<InlineItem>>) -> NodeId {
    let fill = create_memo(move || cx.colors.get().widget);
    let label = create_memo(move || item.get().map(|item| item.label).unwrap_or_default());
    let color = create_memo(move || {
        item.get()
            .map_or(Color::TRANSPARENT, |item| item.style.color)
    });
    let chip = create_memo(move || cx.geometry.get().run_chip);
    let height = create_memo(move || Extent::px(chip.get().height));
    let radius = create_memo(move || chip.get().radius);
    let padding = create_memo(move || Sides::horizontal(chip.get().padding));
    let font = create_memo(move || FontId::monospace(chip.get().font_size));
    view! {
        <Frame height fill=fill radius padding>
            <Text font color={color} align=TextAlign::Center vertical_align=TextAlign::Center>
                {label}
            </Text>
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
    let icon = create_memo(move || widget.get().and_then(|widget| widget.icon));
    let has_icon = create_memo(move || icon.get().is_some());
    let label = create_memo(move || item.get().map(|item| item.label).unwrap_or_default());
    let color = create_memo(move || {
        item.get()
            .map_or(Color::TRANSPARENT, |item| item.style.color)
    });
    let font = create_memo(move || {
        item.get()
            .map_or(FontId::proportional(cx.font_size.get()), |item| {
                item.style.font
            })
    });
    let shape = create_memo(move || cx.geometry.get().widget);
    let height = create_memo(move || Extent::px(shape.get().height));
    let radius = create_memo(move || shape.get().radius);
    let icon_width = create_memo(move || Extent::px(shape.get().icon_width));
    let icon_font = create_memo(move || FontId::proportional(shape.get().icon_size));
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
    view! {
        <Frame @node_ref=&pill height fill=fill radius>
            <List direction=beui_view::types::Direction::Horizontal spacing=0.0>
                <Show condition={has_icon}>||
                    <Frame width=icon_width>
                        <Text font=icon_font color={color} align=TextAlign::Center>
                            <Icon glyph={icon} />
                        </Text>
                    </Frame>
                </Show>
                <Text font={font} color={color} vertical_align=TextAlign::Center>{label}</Text>
                <Show condition={selected}>|| {
                    let render = cx
                        .selected_widget
                        .expect("a widget only offers a popup when the area was given one");
                    let index = index.get_untracked().unwrap_or_default();
                    view! {
                        <WidgetPopup anchor={pill} render index />
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
    let geometry = cx.geometry;
    let above = create_memo(move || {
        let (left, right) = geometry.get().gutter_padding;
        Sides::new(left, model.get().space.0, right, 0.0)
    });
    let arrow_size = create_memo(move || geometry.get().gutter_arrow);
    let arrow_width = create_memo(move || Extent::px(arrow_size.get()));
    let arrow_font = create_memo(move || FontId::proportional(arrow_size.get()));
    let number_font = create_memo(move || FontId::monospace(cx.font_size.get()));
    let arrow = create_memo(move || {
        cx.state.content().get();
        let start = model.get().start;
        cx.state
            .sections()
            .iter()
            .find(|section| section.line_start == start)
            .map(|section| match section.collapsed || section.revealed {
                true => ICON_CHEVRON_RIGHT,
                false => ICON_CHEVRON_DOWN,
            })
    });
    let arrow_color = create_memo(move || cx.colors.get().gutter_arrow);
    let number_color = create_memo(move || cx.colors.get().gutter_text);
    view! {
        <Frame padding=above>
            <List direction=beui_view::types::Direction::Horizontal spacing=0.0>
                <Frame width=arrow_width height=height>
                    <Text font=arrow_font color={arrow_color} align=TextAlign::Center>
                        <Icon glyph={arrow} />
                    </Text>
                </Frame>
                <Frame @sizing=ItemSize::Percent(100.0) height=height>
                    <Text font=number_font color={number_color} align=TextAlign::End>{number}</Text>
                </Frame>
            </List>
        </Frame>
    }
}
