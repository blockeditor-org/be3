pub mod colors;
pub mod keys;
pub mod rows;
pub mod state;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use accesskit::{Node, Role};

use text_editor_core::{
    CursorHorizontalPositionMetric, CursorLeftRightStop, DragSelectionMode, EditorCommand,
    LRDirection, MarkdownCommand, MoveMode, UDDirection, VerticalMoveMode,
};

use beui_macros::{component, view};

use crate::{Edge, Floating, Scroll};
use beui_core::base::{ImeCursor, ItemSize, ScrollPosition};
use beui_core::color::Color32;
use beui_core::document::Document;
use beui_core::font::{FontId, TextAlign};
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::icons::{ICON_CHECK, ICON_KEYBOARD_ARROW_DOWN, ICON_KEYBOARD_ARROW_RIGHT};
use beui_core::input::{CursorIcon, KeyPress, PointerPress};
use beui_core::base::text::TextGeometry;
use beui_core::node::{NodeId, Rects};
use beui_core::rich::{CaretHandle, TextCaret, TextMark, handle_center};
use beui_view::reactive::{
    Callback, Canvas, CanvasItem, Child, ClickCallback, ClickCatcher, Focusable, ForEach, Frame,
    List, Memo, NodeRef, Portal, Prop, ReadSignal, Render, RenderFn, Show, Text, TextItem,
    VirtualList, WriteSignal, clone, component_accessibility, component_rect, component_size,
    create_effect, create_memo, create_signal, create_timer, in_new_scope, on_cleanup,
    try_with_document,
    pixels_per_point, set_component_state, untrack, with_document,
};

use rows::{
    BODY_SIZE, Composition, DOCUMENT_PADDING, INLINE_WIDGET_HEIGHT, INLINE_WIDGET_ICON_INSET,
    Inline, InlineItem, LINE_PADDING, Row, RowInputs, RowOptions, build_row, galley, line_of,
    line_range, line_starts, rich_layout, table_spacers,
};
use state::{AreaGeometry, Grab};

pub use colors::{SyntaxColors, TextAreaColors};
pub use rows::TextWidget;
pub use state::{TextAreaLayout, TextAreaState};

pub const PADDING: Vec2 = Vec2::new(12.0, 8.0);
pub const CARET_WIDTH: f32 = 2.0;
const GUTTER_TEXT_SIZE: f32 = 12.0;
const GUTTER_PADDING_LEFT: f32 = 10.0;
const GUTTER_PADDING_RIGHT: f32 = 10.0;
const GUTTER_ARROW_SIZE: f32 = 14.0;
const TOUCH_HANDLE_HIT_RADIUS: f32 = 24.0;
const CHECKBOX_RADIUS: u8 = 3;
const CHECKBOX_OUTLINE: f32 = 1.5;
const INLINE_WIDGET_RADIUS: u8 = 5;
const REMOTE_SELECTION_ALPHA: u8 = 70;
const CODE_OUTSET: Vec2 = Vec2::new(3.0, -1.0);
const CODE_RADIUS: f32 = 3.0;
const REVEAL_MARGIN: Vec2 = Vec2::new(8.0, 3.0);
const REVEAL_ATTEMPTS: u8 = 3;
const SELECT_ALL_CLICKS: u32 = 4;
const WORD_CLICKS: u32 = 2;
const LINE_CLICKS: u32 = 3;
const HANDLE_SLACK: f32 = 0.5;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteTextCursor {
    pub selection: Range<usize>,
    pub caret: usize,
    pub color: Color32,
}

type GeometryCell = Rc<RefCell<Option<TextGeometry>>>;

#[derive(Clone)]
struct RowEntry {
    id: u64,
    row: NodeRef,
    geometry: GeometryCell,
    model: Memo<Rc<Row>>,
    block: NodeRef,
}

impl RowEntry {
    fn row(&self) -> Rc<Row> {
        self.model.get_untracked()
    }

    fn rect(&self, rects: &Rects) -> Option<Rect> {
        rects.get(&self.row.try_get()?)
    }

    fn text(&self) -> Option<TextGeometry> {
        self.geometry.borrow().clone()
    }
}

struct Surface {
    state: TextAreaState,
    single_line: bool,
    rows: RefCell<BTreeMap<usize, RowEntry>>,
    registered: Cell<u64>,
    rects: Rc<Rects>,
    starts: Memo<Rc<Vec<usize>>>,
    visible: Memo<Vec<usize>>,
    tables: Memo<Rc<HashMap<usize, Vec<(usize, f32)>>>>,
    widgets: Memo<Vec<TextWidget>>,
    colors: Memo<TextAreaColors>,
    composition: Memo<Option<Composition>>,
    placeholder: Memo<String>,
    font_size: Memo<f32>,
    metrics: Memo<(f32, u64)>,
    gutter: Memo<f32>,
    padding: Memo<Vec2>,
    scroll: ReadSignal<ScrollPosition>,
    set_offset: WriteSignal<f32>,
    shift: ReadSignal<f32>,
    set_shift: WriteSignal<f32>,
    view_width: Memo<f32>,
    view_height: Memo<f32>,
    focused: ReadSignal<bool>,
    set_focused: WriteSignal<bool>,
    preedit: ReadSignal<String>,
    set_preedit: WriteSignal<String>,
    set_autoscroll: WriteSignal<bool>,
    viewport: NodeRef,
    list: NodeRef,
    anchor: NodeId,
    masked: Memo<bool>,
    disabled: Memo<bool>,
    remote_cursors: Memo<Vec<RemoteTextCursor>>,
    drop_caret: Memo<Option<usize>>,
    reveal_attempts: Cell<u8>,
    block: Option<RenderFn<usize>>,
    selected_widget: Option<RenderFn<usize>>,
    on_widget_press: Callback<usize, bool>,
    on_menu: Callback<Pos2>,
    on_submit: ClickCallback,
}

type Context = Rc<Surface>;

struct Parts {
    cx: Context,
    shown: Memo<String>,
}

impl AreaGeometry for Surface {
    fn caret_rect(&self, byte: usize) -> Option<Rect> {
        let canvas = self.canvas_rect()?;
        match self.caret_rect_at(byte) {
            Some(rect) => Some(rect.translate(-canvas.min.to_vec2())),
            None => {
                let y = self.estimated_y(byte)?;
                Some(Rect::from_min_size(
                    Pos2::new(PADDING.x, y),
                    Vec2::new(CARET_WIDTH, self.row_estimate()),
                ))
            }
        }
    }

    fn index_at(&self, pos: Pos2) -> Option<usize> {
        self.hit(pos)
    }
}

impl Surface {
    fn canvas_rect(&self) -> Option<Rect> {
        self.rects.get(&self.state.canvas().try_get()?)
    }

    fn node_rect(&self, node: &NodeRef) -> Option<Rect> {
        self.rects.get(&node.try_get()?)
    }

    fn row_estimate(&self) -> f32 {
        let body = galley("", FontId::proportional(self.font_size.get_untracked()))
            .map_or(BODY_SIZE * 1.15, |galley| galley.line_height());
        match self.single_line {
            true => body,
            false => body + LINE_PADDING.0 + LINE_PADDING.1,
        }
    }

    fn line_of(&self, byte: usize) -> usize {
        line_of(&self.starts.get_untracked(), byte)
    }

    fn entry(&self, line: usize) -> Option<RowEntry> {
        self.rows.borrow().get(&line).cloned()
    }

    fn caret_rect_at(&self, byte: usize) -> Option<Rect> {
        let entry = self.entry(self.line_of(byte))?;
        let display = entry.row().to_display(byte);
        entry.text()?.caret_rect(display, CARET_WIDTH)
    }

    fn estimated_y(&self, byte: usize) -> Option<f32> {
        let line = self.line_of(byte);
        let list = self.list.try_get()?;
        let canvas = self.canvas_rect()?;
        let rect = self.rects.get(&list)?;
        let offset = try_with_document(|document| document.virtual_list_offset::<usize>(list, &line))??;
        Some(rect.min.y - canvas.min.y + offset)
    }

    fn row_at(&self, pos: Pos2) -> Option<RowEntry> {
        let rows = self.rows.borrow();
        let placed: Vec<(Rect, &RowEntry)> = rows
            .values()
            .filter_map(|entry| Some((entry.rect(&self.rects)?, entry)))
            .collect();
        placed
            .iter()
            .find(|(rect, _)| pos.y < rect.max.y)
            .or(placed.last())
            .map(|(_, entry)| (*entry).clone())
    }

    fn shifted(&self, text: &TextGeometry, pos: Pos2) -> Pos2 {
        if !self.single_line {
            return pos;
        }
        let (Some(canvas), Some(laid)) = (self.canvas_rect(), text.rect()) else {
            return pos;
        };
        let wanted = canvas.min.x + self.padding.get_untracked().x - self.shift.get_untracked();
        Pos2::new(pos.x + laid.min.x - wanted, pos.y)
    }

    fn hit(&self, pos: Pos2) -> Option<usize> {
        let entry = self.row_at(pos)?;
        let text = entry.text()?;
        let display = text.index_at(self.shifted(&text, pos))?;
        Some(entry.row().to_source(display))
    }

    fn inline_at(&self, pos: Pos2) -> Option<(RowEntry, InlineItem)> {
        let entry = self.row_at(pos)?;
        let text = entry.text()?;
        let index = text.inline_at(self.shifted(&text, pos))?;
        let item = entry.row().inline.get(index)?.clone();
        Some((entry, item))
    }

    fn gutter_arrow_at(&self, pos: Pos2) -> Option<usize> {
        let gutter = self.gutter.get_untracked();
        let entry = self.row_at(pos)?;
        let rect = entry.rect(&self.rects)?;
        if pos.x < rect.min.x || pos.x > rect.min.x + gutter {
            return None;
        }
        let row = entry.row();
        let start = row.start;
        let collapsible = self
            .state
            .sections()
            .iter()
            .any(|section| section.line_start == start);
        let arrow = Rect::from_min_size(
            Pos2::new(
                rect.min.x + GUTTER_PADDING_LEFT,
                rect.min.y + (row.line_height - GUTTER_ARROW_SIZE) / 2.0,
            ),
            Vec2::splat(GUTTER_ARROW_SIZE),
        );
        (collapsible && arrow.contains(pos)).then_some(start)
    }

    fn block_at(&self, pos: Pos2) -> Option<usize> {
        let rows = self.rows.borrow();
        rows.values().find_map(|entry| {
            let (index, _) = entry.row().block?;
            let rect = self.node_rect(&entry.block)?;
            rect.contains(pos).then_some(index)
        })
    }

    fn reveal_caret(&self) {
        if self.single_line {
            self.state.take_reveal();
            if self.state.take_reveal_cursor() {
                self.reveal_across();
            }
            return;
        }
        let position = self.scroll.get_untracked();
        if position.viewport <= 0.0 {
            return;
        }
        if let Some(rect) = self.state.take_reveal() {
            self.reveal_rect(rect);
        }
        if !self.state.take_reveal_cursor() {
            self.reveal_attempts.set(0);
            return;
        }
        let Some(byte) = self.state.caret_indices().first().copied() else {
            return;
        };
        let canvas = self.canvas_rect();
        let (rect, exact) = match (self.caret_rect_at(byte), canvas) {
            (Some(rect), Some(canvas)) => (rect.translate(-canvas.min.to_vec2()), true),
            _ => match self.estimated_y(byte) {
                Some(y) => (
                    Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(0.0, self.row_estimate())),
                    false,
                ),
                None => return,
            },
        };
        let mut offset = position.offset;
        if rect.min.y - REVEAL_MARGIN.y < offset {
            offset = rect.min.y - REVEAL_MARGIN.y;
        }
        if rect.max.y + REVEAL_MARGIN.y > offset + position.viewport {
            offset = rect.max.y + REVEAL_MARGIN.y - position.viewport;
        }
        self.set_offset
            .set_unconditionally(offset.clamp(0.0, position.max_offset().max(offset)));
        let attempts = self.reveal_attempts.get();
        if !exact && attempts < REVEAL_ATTEMPTS {
            self.reveal_attempts.set(attempts + 1);
            self.state.reveal_cursor();
        } else {
            self.reveal_attempts.set(0);
        }
    }

    fn reveal_rect(&self, rect: Rect) {
        let position = self.scroll.get_untracked();
        let middle = rect.center().y - position.viewport / 2.0;
        self.set_offset
            .set_unconditionally(middle.clamp(0.0, position.max_offset()));
    }

    fn caret_x(&self, byte: usize) -> Option<f32> {
        let entry = self.entry(self.line_of(byte))?;
        let text = entry.text()?;
        let caret = text.caret_rect(entry.row().to_display(byte), CARET_WIDTH)?;
        Some(caret.min.x - text.rect()?.min.x)
    }

    fn reveal_across(&self) {
        let Some(x) = self
            .state
            .caret_indices()
            .first()
            .and_then(|byte| self.caret_x(*byte))
        else {
            return;
        };
        let room = self.room();
        let width = self.text_size().x;
        let mut shift = self.shift.get_untracked();
        if x < shift {
            shift = x;
        }
        if x > shift + room {
            shift = x - room;
        }
        self.set_shift
            .set(shift.clamp(0.0, (width - room).max(0.0)));
    }

    fn text_size(&self) -> Vec2 {
        let Some(entry) = self.entry(0) else {
            return Vec2::ZERO;
        };
        rich_layout(
            &entry.row(),
            self.font_size.get_untracked(),
            (0.0, 0.0),
            f32::INFINITY,
        )
        .map_or(Vec2::ZERO, |layout| layout.size)
    }

    fn room(&self) -> f32 {
        (self.view_width.get_untracked() - self.padding.get_untracked().x * 2.0 - CARET_WIDTH)
            .max(0.0)
    }

    fn selection_handles(&self) -> Option<Range<usize>> {
        if !self.state.touch_mode().get_untracked() {
            return None;
        }
        let range = self.state.selection_ranges().into_iter().next()?;
        (range.start != range.end).then_some(range)
    }

    fn caret_handle(&self) -> Option<usize> {
        let state = &self.state;
        if !state.touch_mode().get_untracked()
            || !state.caret_handle().get_untracked()
            || !self.focused.get_untracked()
            || state
                .selection_ranges()
                .iter()
                .any(|range| !range.is_empty())
        {
            return None;
        }
        state.caret_indices().first().copied()
    }

    fn handles(&self) -> Vec<(CaretHandle, usize)> {
        match (self.caret_handle(), self.selection_handles()) {
            (Some(caret), _) => vec![(CaretHandle::Middle, caret)],
            (None, Some(range)) => vec![
                (CaretHandle::Start, range.start),
                (CaretHandle::End, range.end),
            ],
            (None, None) => Vec::new(),
        }
    }

    fn shown_handles(&self) -> Vec<(CaretHandle, usize)> {
        self.handles()
            .into_iter()
            .filter(|(_, byte)| self.in_view(*byte))
            .collect()
    }

    fn in_view(&self, byte: usize) -> bool {
        let Some(caret) = self.caret_rect_at(byte) else {
            return false;
        };
        let Some(viewport) = self.node_rect(&self.viewport) else {
            return true;
        };
        match self.single_line {
            true => {
                (viewport.min.x - HANDLE_SLACK..=viewport.max.x + HANDLE_SLACK)
                    .contains(&caret.min.x)
            }
            false => caret.max.y > viewport.min.y && caret.min.y < viewport.max.y,
        }
    }

    fn handle_at(&self, pos: Pos2) -> Option<CaretHandle> {
        self.shown_handles()
            .into_iter()
            .filter_map(|(handle, byte)| {
                let caret = self.caret_rect_at(byte)?;
                if pos.y < caret.max.y {
                    return None;
                }
                let center = handle_center(caret, handle);
                let distance = (Vec2::new(pos.x, pos.y) - center).length();
                (distance <= TOUCH_HANDLE_HIT_RADIUS).then_some((handle, distance))
            })
            .min_by(|left, right| left.1.total_cmp(&right.1))
            .map(|(handle, _)| handle)
    }

    fn inside(&self, pos: Pos2) -> (Pos2, Option<Beyond>) {
        let Some(rect) = self.node_rect(&self.viewport) else {
            return (pos, None);
        };
        let (along, start, end) = match self.single_line {
            true => (pos.x, rect.min.x, rect.max.x),
            false => (pos.y, rect.min.y, rect.max.y),
        };
        let end = (end - 1.0).max(start);
        let beyond = if along < start {
            Some(Beyond::Before)
        } else if along > end {
            Some(Beyond::After)
        } else {
            None
        };
        let along = along.clamp(start, end);
        let inside = match self.single_line {
            true => Pos2::new(along, pos.y),
            false => Pos2::new(pos.x, along),
        };
        (inside, beyond)
    }

    fn step_beyond(&self, beyond: Option<Beyond>, mode: MoveMode) {
        self.set_autoscroll.set(beyond.is_some());
        let Some(beyond) = beyond else {
            return;
        };
        if self.single_line {
            self.state.execute(EditorCommand::MoveCursorLeftRight {
                mode,
                direction: match beyond {
                    Beyond::Before => LRDirection::Left,
                    Beyond::After => LRDirection::Right,
                },
                stop: CursorLeftRightStop::UnicodeGraphemeCluster,
            });
            return;
        }
        self.state.execute(EditorCommand::MoveCursorUpDown {
            direction: match beyond {
                Beyond::Before => UDDirection::Up,
                Beyond::After => UDDirection::Down,
            },
            mode: match mode {
                MoveMode::Select => VerticalMoveMode::Select,
                MoveMode::Move => VerticalMoveMode::Move,
            },
            metric: CursorHorizontalPositionMetric::Byte,
            stop: CursorLeftRightStop::UnicodeGraphemeCluster,
        });
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Layer {
    All,
    Text,
    Handles,
}

#[derive(Clone, Copy)]
enum Beyond {
    Before,
    After,
}

fn begin_handle_drag(cx: &Context, handle: CaretHandle, pos: Pos2) {
    let (grab, moving_byte) = match handle {
        CaretHandle::Middle => {
            let Some(caret) = cx.caret_handle() else {
                return;
            };
            (Grab::Caret, caret)
        }
        CaretHandle::Start | CaretHandle::End => {
            let Some(range) = cx.selection_handles() else {
                return;
            };
            let (fixed, moving) = match handle {
                CaretHandle::Start => (range.end, range.start),
                _ => (range.start, range.end),
            };
            (Grab::Selection(cx.state.core().position(fixed)), moving)
        }
    };
    let offset = cx
        .caret_rect_at(moving_byte)
        .map_or(Vec2::ZERO, |caret| pos - caret.min);
    cx.state.begin_grab(grab, offset);
}

fn drag_handle(cx: &Context, pos: Pos2) {
    let Some(grab) = cx.state.grab() else {
        return;
    };
    let offset = cx.state.grab_offset();
    let (inside, beyond) = cx.inside(pos - offset);
    let Some(target) = cx.hit(inside) else {
        return;
    };
    let position = cx.state.core().position(target);
    cx.state.execute(match grab {
        Grab::Selection(fixed) => EditorCommand::DragSelectionHandle { fixed, position },
        Grab::Caret => EditorCommand::SetSelection {
            anchor: position,
            focus: position,
        },
    });
    cx.step_beyond(
        beyond,
        match grab {
            Grab::Selection(_) => MoveMode::Select,
            Grab::Caret => MoveMode::Move,
        },
    );
    cx.state.reveal_cursor();
}

fn press(cx: &Context, press: PointerPress) {
    if cx.disabled.get_untracked() {
        return;
    }
    cx.set_focused.set(true);
    cx.state.set_touch_mode(press.touch);
    cx.state.end_grab();
    if press.touch
        && let Some(handle) = cx.handle_at(press.pos)
    {
        begin_handle_drag(cx, handle, press.pos);
        return;
    }
    if !press.touch {
        cx.state.set_caret_handle(false);
    }
    if let Some(widget) = cx.block_at(press.pos)
        && cx.on_widget_press.call(widget)
    {
        cx.state.set_selecting(false);
        return;
    }
    if let Some(line_start) = cx.gutter_arrow_at(press.pos) {
        let position = cx.state.core().position(line_start);
        cx.state.execute(EditorCommand::ToggleCollapseAt(position));
        return;
    }
    if press.touch {
        return;
    }
    select_at(
        cx,
        press.pos,
        press.clicks,
        press.modifiers.shift,
        !cx.single_line && press.modifiers.alt != press.modifiers.ctrl,
    );
}

fn tap(cx: &Context, press: PointerPress) {
    if !press.touch || cx.disabled.get_untracked() {
        return;
    }
    match cx.state.end_grab() {
        Some(Grab::Caret) => {
            cx.on_menu.call(press.pos);
            return;
        }
        Some(Grab::Selection(_)) => return,
        None => {}
    }
    let Some(target) = cx.hit(press.pos) else {
        return;
    };
    if cx.state.selection_contains(target) {
        cx.on_menu.call(press.pos);
        cx.state.set_selecting(false);
        return;
    }
    cx.state.set_caret_handle(true);
    select_at(cx, press.pos, press.clicks, false, false);
    cx.state.set_selecting(false);
}

fn select_at(cx: &Context, pos: Pos2, clicks: u32, extend: bool, syntax: bool) {
    match cx.inline_at(pos) {
        Some((_, item)) => match item.inline {
            Inline::Checkbox { line_start, .. } => {
                let position = cx.state.core().position(line_start);
                cx.state
                    .execute(EditorCommand::Markdown(MarkdownCommand::ToggleCheckbox(
                        position,
                    )));
                cx.state.set_selecting(false);
                return;
            }
            Inline::Widget(index) => {
                let Some(widget) = cx.widgets.get_untracked().get(index).cloned() else {
                    return;
                };
                let (anchor, focus) = {
                    let core = cx.state.core();
                    (
                        core.position(widget.range.start),
                        core.position(widget.range.end),
                    )
                };
                cx.state
                    .execute(EditorCommand::SetSelection { anchor, focus });
                cx.state.set_selecting(false);
                return;
            }
        },
        None => {}
    }
    let Some(target) = cx.hit(pos) else {
        return;
    };
    if clicks >= SELECT_ALL_CLICKS {
        cx.state.execute(EditorCommand::SelectAll);
        cx.state.set_selecting(false);
        return;
    }
    let mode = match clicks {
        WORD_CLICKS => DragSelectionMode::select(CursorLeftRightStop::Word),
        LINE_CLICKS => DragSelectionMode::select(CursorLeftRightStop::Line),
        _ => DragSelectionMode::default(),
    };
    let position = cx.state.core().position(target);
    cx.state.execute(EditorCommand::Click {
        position,
        mode,
        extend,
        select_syntax_node: syntax,
    });
    cx.state.set_selecting(true);
    cx.state.reveal_cursor();
}

fn extend(cx: &Context, press: PointerPress) {
    if cx.disabled.get_untracked() {
        return;
    }
    if cx.state.grab().is_some() {
        drag_handle(cx, press.pos);
        return;
    }
    if !cx.state.selecting() || press.touch {
        return;
    }
    let (inside, beyond) = cx.inside(press.pos);
    let Some(target) = cx.hit(inside) else {
        return;
    };
    let position = cx.state.core().position(target);
    cx.state.execute(EditorCommand::Drag(position));
    cx.step_beyond(beyond, MoveMode::Select);
    cx.state.reveal_cursor();
}

fn blur(cx: &Context) {
    cx.set_preedit.set(String::new());
    cx.state.end_grab();
    cx.state.set_selecting(false);
    cx.set_autoscroll.set(false);
    cx.state.external_edit();
}

fn release(cx: &Context, active: bool) {
    if active {
        return;
    }
    cx.state.set_selecting(false);
    cx.set_autoscroll.set(false);
}

fn insert_text(cx: &Context, text: &str) {
    if cx.disabled.get_untracked() {
        return;
    }
    let text = match cx.single_line {
        true => text.chars().filter(|letter| !letter.is_control()).collect(),
        false if text == "\n" || text == "\r" => String::new(),
        false => text.to_owned(),
    };
    if text.is_empty() {
        return;
    }
    cx.state.set_caret_handle(false);
    cx.state.execute(EditorCommand::InsertText(text.as_bytes()));
    cx.state.reveal_cursor();
}

fn compose(cx: &Context, text: String) {
    let text = match cx.disabled.get_untracked() {
        true => String::new(),
        false => text.chars().filter(|letter| !letter.is_control()).collect(),
    };
    if text == cx.preedit.get_untracked() {
        return;
    }
    cx.set_preedit.set(text);
    cx.state.set_caret_handle(false);
    cx.state.reveal_cursor();
}

fn gutter_width(line_count: usize) -> f32 {
    let digit = galley("0", FontId::monospace(GUTTER_TEXT_SIZE))
        .map_or(GUTTER_TEXT_SIZE * 0.6, |galley| galley.size().x);
    let mut digits = 1;
    let mut value = line_count.max(1);
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    GUTTER_PADDING_LEFT + GUTTER_ARROW_SIZE + digit * digits as f32 + GUTTER_PADDING_RIGHT
}

#[derive(Clone)]
struct Field {
    cx: Context,
    described: Memo<Node>,
    frame: Rc<RefCell<Option<Render<Child>>>>,
    inner: NodeRef,
    surface_color: Memo<Color32>,
    set_field_rect: WriteSignal<Rect>,
    cursor: Memo<CursorIcon>,
    set_cursor: WriteSignal<CursorIcon>,
    autoscroll: ReadSignal<bool>,
    tab_stop: Memo<bool>,
    ime_cursor: Memo<Option<ImeCursor>>,
    on_key_override: Callback<KeyPress, bool>,
    on_focus_change: Callback<bool>,
    on_hover_change: Callback<bool>,
}

#[component]
pub fn TextArea(
    state: TextAreaState,
    #[prop(default = false)] single_line: bool,
    #[prop(default = Vec::new())] widgets: Prop<Vec<TextWidget>>,
    #[prop(default = TextAreaColors::DEFAULT)] colors: Prop<TextAreaColors>,
    #[prop(default = Vec::new())] remote_cursors: Prop<Vec<RemoteTextCursor>>,
    #[prop(default = None)] drop_caret: Prop<Option<usize>>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] password: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = false)] disabled: Prop<bool>,
    #[prop(default = BODY_SIZE)] font_size: Prop<f32>,
    #[prop(default = PADDING)] padding: Prop<Vec2>,
    accessibility: Option<Prop<Node>>,
    frame: Option<Render<Child>>,
    block: Option<RenderFn<usize>>,
    selected_widget: Option<RenderFn<usize>>,
    on_widget_press: Callback<usize, bool>,
    on_menu: Callback<Pos2>,
    on_key_override: Callback<KeyPress, bool>,
    on_submit: ClickCallback,
    on_focus_change: Callback<bool>,
    on_hover_change: Callback<bool>,
) -> NodeId {
    let focus_request = focused;
    let size = component_size();
    let (field_rect, set_field_rect) = create_signal(Rect::ZERO);
    let placed = match single_line {
        true => field_rect,
        false => component_rect(),
    };
    let scale = pixels_per_point();
    let attached = with_document(|document| document.watch_context());
    let (scroll, set_scroll) = create_signal(ScrollPosition::ZERO);
    let (offset, set_offset) = create_signal(0.0_f32);
    let (shift, set_shift) = create_signal(0.0_f32);
    let (focused, set_focused) = create_signal(false);
    let (preedit, set_preedit) = create_signal(String::new());
    let (autoscroll, set_autoscroll) = create_signal(false);
    let viewport = NodeRef::new();
    let (outer, inner) = match single_line {
        true => (NodeRef::new(), viewport.clone()),
        false => (viewport.clone(), NodeRef::new()),
    };
    let masked = create_memo(move || password.get());
    let placeholder = create_memo(move || placeholder.get());
    let disabled = create_memo(move || disabled.get());
    let font_size = create_memo(move || font_size.get());
    let padding = create_memo(move || padding.get());
    let widgets = create_memo(move || widgets.get());
    let colors = create_memo(move || colors.get());
    let remote_cursors = create_memo(move || remote_cursors.get());
    let drop_caret = create_memo(move || drop_caret.get());
    let view_width = create_memo(clone!(placed -> move || placed.get().width()));

    let role = match single_line {
        true => Role::TextInput,
        false => Role::MultilineTextInput,
    };
    let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(role)));
    let accessible = state.content();
    let described = create_memo(
        clone!(state accessible masked placeholder disabled -> move || {
            accessible.get();
            let mut node = accessibility.get();
            let value = String::from_utf8_lossy(&state.bytes()).into_owned();
            node.set_value(match masked.get() {
                true => rows::mask(&value),
                false => value,
            });
            let placeholder = placeholder.get();
            if !placeholder.is_empty() {
                node.set_placeholder(placeholder);
            }
            match disabled.get() {
                true => node.set_disabled(),
                false => node.clear_disabled(),
            }
            node
        }),
    );

    let content = state.content();
    let starts = create_memo(clone!(state content -> move || {
        content.get();
        Rc::new(line_starts(&state.bytes()))
    }));
    let visible = create_memo(clone!(state content starts -> move || {
        content.get();
        let starts = starts.get();
        state.with_snapshot(|snapshot| {
            starts
                .iter()
                .enumerate()
                .filter(|(_, start)| !snapshot.hidden.iter().any(|range| range.contains(*start)))
                .map(|(line, _)| line)
                .collect::<Vec<usize>>()
        })
    }));
    let gutter = create_memo(clone!(scale attached starts -> move || {
        scale.get();
        attached.get();
        match single_line {
            true => 0.0,
            false => gutter_width(starts.get().len()),
        }
    }));
    let tables = create_memo(
        clone!(state content starts widgets colors masked font_size scale attached -> move || {
            content.get();
            scale.get();
            attached.get();
            let starts = starts.get();
            let widgets = widgets.get();
            let colors = colors.get();
            let options = RowOptions {
                body_size: font_size.get(),
                mask: masked.get(),
                single_line,
            };
            Rc::new(state.with_snapshot(|snapshot| match snapshot.loaded && !single_line {
                true => table_spacers(&rows::TableInputs {
                    snapshot,
                    widgets: &widgets,
                    colors: &colors,
                    options,
                    starts: &starts,
                }),
                false => HashMap::new(),
            }))
        }),
    );
    let composition = create_memo(clone!(state preedit -> move || {
        let text = preedit.get();
        if text.is_empty() {
            return None;
        }
        state.cursors().get();
        Some(Composition {
            at: *state.caret_indices().first()?,
            text,
        })
    }));
    let anchor = in_new_scope(|| view! { <Frame width=CARET_WIDTH /> });
    on_cleanup(move || {
        beui_core::current::try_with_document(|document| document.remove_node(anchor));
    });

    let cx: Context = Rc::new(Surface {
        state: state.clone(),
        single_line,
        rows: RefCell::new(BTreeMap::new()),
        registered: Cell::new(0),
        rects: with_document(|document| Rc::clone(&document.rects)),
        starts: starts.clone(),
        visible: visible.clone(),
        tables,
        widgets: widgets.clone(),
        colors: colors.clone(),
        composition,
        placeholder: placeholder.clone(),
        font_size: font_size.clone(),
        metrics: create_memo(clone!(scale attached -> move || (scale.get(), attached.get()))),
        gutter: gutter.clone(),
        padding: padding.clone(),
        scroll: scroll.clone(),
        set_offset,
        shift: shift.clone(),
        set_shift: set_shift.clone(),
        view_width: view_width.clone(),
        view_height: create_memo(clone!(placed -> move || placed.get().height())),
        focused: focused.clone(),
        set_focused: set_focused.clone(),
        preedit: preedit.clone(),
        set_preedit,
        set_autoscroll,
        viewport: viewport.clone(),
        list: NodeRef::new(),
        anchor,
        masked: masked.clone(),
        disabled: disabled.clone(),
        remote_cursors,
        drop_caret,
        reveal_attempts: Cell::new(0),
        block,
        selected_widget,
        on_widget_press,
        on_menu,
        on_submit,
    });
    state.publish_layout(TextAreaLayout::new(cx.clone()));

    if single_line {
        let clamp_cx = cx.clone();
        create_effect(clone!(content view_width shift -> move || {
            content.get();
            view_width.get();
            let (room, width) = untrack(|| (clamp_cx.room(), clamp_cx.text_size().x));
            let most = (width - room).max(0.0);
            if shift.get_untracked() > most {
                set_shift.set(most);
            }
        }));
    }

    let ime_cursor = create_memo(clone!(state focused font_size -> move || {
        state.cursors().get();
        if !focused.get() {
            return None;
        }
        Some(ImeCursor {
            node: anchor,
            rect: None,
        })
        .filter(|_| font_size.get() > 0.0)
    }));

    let focus_requests = state.focus_requests();
    let focus_writer = set_focused.clone();
    create_effect(move || {
        if focus_requests.get() > 0 {
            focus_writer.set(true);
        }
    });
    create_effect(clone!(set_focused -> move || set_focused.set(focus_request.get())));
    let content = state.content();
    let shown = create_memo(clone!(state masked placeholder -> move || {
        content.get();
        let value = String::from_utf8_lossy(&state.bytes()).into_owned();
        match (value.is_empty(), masked.get()) {
            (true, _) => placeholder.get(),
            (false, true) => rows::mask(&value),
            (false, false) => value,
        }
    }));
    set_component_state(Parts {
        cx: cx.clone(),
        shown,
    });
    let reveal_cx = cx.clone();
    let revealing = create_timer(move || {
        reveal_cx.reveal_caret();
        None
    });
    let reveals = state.reveals();
    create_effect(move || {
        reveals.get();
        revealing.start(Duration::ZERO);
    });
    let (hover_cursor, set_cursor) = create_signal(CursorIcon::Text);
    let cursor = create_memo(clone!(disabled -> move || match disabled.get() {
        true => CursorIcon::Default,
        false => hover_cursor.get(),
    }));
    let tab_stop = create_memo(clone!(disabled -> move || !disabled.get()));
    let surface_color = create_memo(clone!(colors -> move || colors.get().surface));
    let outer_color = create_memo(clone!(surface_color -> move || match single_line {
        true => Color32::TRANSPARENT,
        false => surface_color.get(),
    }));
    let field = Field {
        described,
        frame: Rc::new(RefCell::new(frame)),
        inner,
        surface_color,
        set_field_rect,
        cx: cx.clone(),
        cursor,
        set_cursor,
        autoscroll,
        tab_stop,
        ime_cursor,
        on_key_override,
        on_focus_change,
        on_hover_change,
    };
    let scrolled = field.clone();
    let multi_line = !single_line;
    let strip_width = create_memo(clone!(gutter -> move || gutter.get()));
    let width = create_memo(clone!(size -> move || size.get().x));
    let height = create_memo(clone!(size -> move || size.get().y));
    let gutter_color = create_memo(clone!(colors -> move || colors.get().gutter));
    let border_color = create_memo(clone!(colors -> move || colors.get().gutter_border));
    let border_x = create_memo(clone!(gutter -> move || gutter.get() - 1.0));
    let (strip_height, border_height, area_width, area_height) =
        (height.clone(), height.clone(), width.clone(), height.clone());

    view! {
        <Frame @node_ref=&outer color={outer_color}>
            <List spacing=0.0>
                <Show condition={single_line}>
                    {move || view! {
                        <Editing @sizing=ItemSize::Percent(100.0) field />
                    }}
                </Show>
                <Show condition={multi_line}>
                    {move || view! {
                        <Canvas @sizing=ItemSize::Percent(100.0) width={width} height={height}>
                            <CanvasItem x=0.0 y=0.0 width={strip_width} height={strip_height}>
                                <Frame color={gutter_color} />
                            </CanvasItem>
                            <CanvasItem x={border_x} y=0.0 width=1.0 height={border_height}>
                                <Frame color={border_color} />
                            </CanvasItem>
                            <CanvasItem x=0.0 y=0.0 width={area_width} height={area_height}>
                                <Scroll
                                    offset={offset}
                                    focus_color={Color32::TRANSPARENT}
                                    on_change={move |position: ScrollPosition| set_scroll.set(position)}
                                >
                                    <Editing field={scrolled} />
                                </Scroll>
                            </CanvasItem>
                        </Canvas>
                    }}
                </Show>
            </List>
        </Frame>
    }
}

#[component]
fn Editing(field: Field) -> NodeId {
    let Field {
        cx,
        described,
        frame,
        inner,
        surface_color,
        set_field_rect,
        cursor,
        set_cursor,
        autoscroll,
        tab_stop,
        ime_cursor,
        on_key_override,
        on_focus_change,
        on_hover_change,
    } = field;
    let preedit_cx = cx.clone();
    let focused = cx.focused.clone();
    let set_focused = cx.set_focused.clone();
    let (blur_cx, text_cx, key_cx, capture_cx) = (cx.clone(), cx.clone(), cx.clone(), cx.clone());
    let (press_cx, tap_cx, drag_cx, release_cx) = (cx.clone(), cx.clone(), cx.clone(), cx.clone());
    component_accessibility(described);
    let single_line = cx.single_line;
    let disabled = cx.disabled.clone();
    let hover_cx = cx.clone();
    let body_cx = cx;
    view! {
        <Focusable
            focused
            tab_stop
            ime={create_memo(move || !disabled.get())}
            ime_cursor
            on_focus_change={move |is_focused: bool| {
                set_focused.set(is_focused);
                if !is_focused {
                    blur(&blur_cx);
                }
                on_focus_change.call(is_focused);
            }}
            on_text={move |typed: String| insert_text(&text_cx, &typed)}
            on_preedit={move |text: String| compose(&preedit_cx, text)}
            on_key={move |press: KeyPress| {
                !key_cx.preedit.get_untracked().is_empty()
                    || (!key_cx.disabled.get_untracked()
                        && (on_key_override.call(press) || keys::key(&key_cx, press)))
            }}
        >
            <ClickCatcher
                cursor
                repeat_drag={autoscroll}
                capture_at={move |pos: Pos2| {
                    !capture_cx.disabled.get_untracked() && capture_cx.handle_at(pos).is_some()
                }}
                on_press={move |event: PointerPress| press(&press_cx, event)}
                on_click_at={move |event: PointerPress| tap(&tap_cx, event)}
                on_drag={move |event: PointerPress| extend(&drag_cx, event)}
                on_active_change={move |active: bool| release(&release_cx, active)}
                on_hover_change={move |hovered: bool| on_hover_change.call(hovered)}
                on_hover_move={move |event: PointerPress| {
                    set_cursor.set(hover_cursor(&hover_cx, event.pos));
                }}
            >
                {{
                    match single_line {
                        false => view! { <Lines cx={body_cx} /> },
                        true => {
                            let field = view! {
                                <FieldFrame
                                    @node_ref=&inner
                                    color={surface_color}
                                    report={set_field_rect}
                                >
                                    <SingleLine cx={body_cx} />
                                </FieldFrame>
                            };
                            match frame.borrow_mut().take() {
                                Some(frame) => frame.call(field),
                                None => field,
                            }
                        }
                    }
                }}
            </ClickCatcher>
        </Focusable>
    }
}

#[component]
fn Lines(cx: Context) -> NodeId {
    let canvas = cx.state.canvas();
    let fill = create_memo(clone!(cx -> move || Some(cx.view_height.get())));
    let list = cx.list.clone();
    let keys = cx.visible.clone();
    let top = create_memo(clone!(cx -> move || Some(cx.padding.get().y)));
    let bottom = create_memo(clone!(cx -> move || Some((DOCUMENT_PADDING.y - cx.padding.get().y).max(0.0))));
    let estimate = cx.row_estimate();
    view! {
        <Frame @node_ref=&canvas min_height={fill}>
            <List spacing=0.0>
                <Frame height={top} />
                <VirtualList @node_ref=&list keys item_size=estimate>
                    {move |line: usize| view! { <AreaRow cx={cx.clone()} line /> }}
                </VirtualList>
                <Frame height={bottom} />
            </List>
        </Frame>
    }
}

#[component]
fn SingleLine(cx: Context) -> NodeId {
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
    let canvas_height = create_memo(clone!(cx text -> move || text.get().y + cx.padding.get().y * 2.0));
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
            <CanvasItem x={x.clone()} y={y.clone()} width={text_width.clone()} height={text_height.clone()}>
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
    let padding = create_memo(clone!(cx -> move || cx.padding.get().x));
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
            let (start, end, newline) = line_range(&snapshot.bytes, &starts, line).unwrap_or((0, 0, false));
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
                        <Portal node={Some(anchor)} />
                    </TextItem>
                }}
            </Show>
        </Text>
    }
}

#[component]
fn InlineView(cx: Context, model: Memo<Rc<Row>>, index: usize) -> NodeId {
    let item = create_memo(clone!(model -> move || model.get().inline.get(index).cloned()));
    let checkbox = create_memo(clone!(item -> move || matches!(item.get().map(|item| item.inline), Some(Inline::Checkbox { .. }))));
    let widget = create_memo(clone!(checkbox -> move || !checkbox.get()));
    let (checkbox_cx, widget_cx) = (cx.clone(), cx);
    let (checkbox_item, widget_item) = (item.clone(), item);
    view! {
        <List spacing=0.0>
            <Show condition={checkbox}>
                {move || view! { <CheckboxBox cx={checkbox_cx.clone()} item={checkbox_item.clone()} /> }}
            </Show>
            <Show condition={widget}>
                {move || view! { <WidgetPill cx={widget_cx.clone()} item={widget_item.clone()} /> }}
            </Show>
        </List>
    }
}

#[component]
fn CheckboxBox(cx: Context, item: Memo<Option<InlineItem>>) -> NodeId {
    let checked = create_memo(clone!(item -> move || matches!(item.get().map(|item| item.inline), Some(Inline::Checkbox { checked: true, .. }))));
    let fill = create_memo(clone!(cx checked -> move || match checked.get() {
        true => cx.colors.get().widget,
        false => Color32::TRANSPARENT,
    }));
    let outline = create_memo(clone!(cx -> move || cx.colors.get().gutter_arrow));
    let size = rows::CHECKBOX_WIDTH;
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
    let index = create_memo(clone!(item -> move || match item.get().map(|item| item.inline) {
        Some(Inline::Widget(index)) => Some(index),
        _ => None,
    }));
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
    let width = create_memo(clone!(item -> move || Some(item.get().map_or(0.0, |item| item.size.x))));
    let icon = create_memo(clone!(widget -> move || widget.get().and_then(|widget| widget.icon).unwrap_or("").to_owned()));
    let has_icon = create_memo(clone!(icon -> move || !icon.get().is_empty()));
    let label = create_memo(clone!(item -> move || item.get().map(|item| item.label).unwrap_or_default()));
    let color = create_memo(clone!(item -> move || item.get().map_or(Color32::WHITE, |item| item.style.color)));
    let font_size = create_memo(clone!(item -> move || item.get().map_or(BODY_SIZE, |item| item.style.font.size)));
    let bold = create_memo(clone!(item -> move || item.get().is_some_and(|item| item.style.font.bold)));
    let italic = create_memo(clone!(item -> move || item.get().is_some_and(|item| item.style.font.italic)));
    let monospace = create_memo(clone!(item -> move || item.get().is_some_and(|item| item.style.font.family == beui_core::font::FontFamily::Monospace)));
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
                            <Text string={icon.clone()} icon=true font_size=16.0 color={Color32::WHITE} align=TextAlign::Center />
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
                        view! { <WidgetPopup anchor render index /> }
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
        <Floating anchor edge=Edge::Bottom>
            {content}
        </Floating>
    }
}

#[component]
fn BlockSlot(render: RenderFn<usize>, index: usize, size: Vec2) -> NodeId {
    let content = render.call(index);
    view! {
        <Frame width={size.x} height={size.y}>
            {content}
        </Frame>
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
        <List direction=beui_core::base::Direction::Horizontal spacing=0.0>
            <Frame width=GUTTER_PADDING_LEFT />
            <Frame width=GUTTER_ARROW_SIZE height={height.clone()}>
                <Text string={arrow} icon=true font_size=GUTTER_ARROW_SIZE color={arrow_color} align=TextAlign::Center />
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
            <Frame width=GUTTER_PADDING_RIGHT />
        </List>
    }
}

pub fn text_area_shown(document: &Document, area: NodeId) -> String {
    document
        .component_state::<Parts>(area)
        .shown
        .get_untracked()
}

pub fn text_area_index_at(document: &Document, area: NodeId, pos: Pos2) -> usize {
    let cx = document.component_state::<Parts>(area).cx.clone();
    area_hit(document, &cx, pos).unwrap_or(0)
}

fn area_hit(document: &Document, cx: &Context, pos: Pos2) -> Option<usize> {
    let rows = cx.rows.borrow();
    let placed: Vec<(Rect, &RowEntry)> = rows
        .values()
        .filter_map(|entry| Some((entry.rect(&document.rects)?, entry)))
        .collect();
    let (_, entry) = placed
        .iter()
        .find(|(rect, _)| pos.y < rect.max.y)
        .or(placed.last())?;
    let display = entry.text()?.index_at(pos)?;
    Some(entry.row().to_source(display))
}

pub fn text_area_state(document: &Document, area: NodeId) -> TextAreaState {
    document.component_state::<Parts>(area).cx.state.clone()
}

pub fn text_area_handles(document: &Document, area: NodeId) -> Vec<Pos2> {
    let cx = document.component_state::<Parts>(area).cx.clone();
    let rows = cx.rows.borrow();
    cx.handles()
        .into_iter()
        .filter_map(|(handle, byte)| {
            let line = line_of(&cx.starts.get_untracked(), byte);
            let entry = rows.get(&line)?;
            let caret = entry
                .text()?
                .caret_rect(entry.row().to_display(byte), CARET_WIDTH)?;
            let viewport = cx.viewport.try_get().and_then(|node| document.node_rect(node));
            let shown = viewport.is_none_or(|viewport| match cx.single_line {
                true => (viewport.min.x - HANDLE_SLACK..=viewport.max.x + HANDLE_SLACK)
                    .contains(&caret.min.x),
                false => caret.max.y > viewport.min.y && caret.min.y < viewport.max.y,
            });
            let center = handle_center(caret, handle);
            shown.then_some(Pos2::new(center.x, center.y))
        })
        .collect()
}

#[component]
fn FieldFrame(color: Memo<Color32>, report: WriteSignal<Rect>, children: Child) -> NodeId {
    let placed = component_rect();
    create_effect(move || report.set(placed.get()));
    view! {
        <Frame color>{children}</Frame>
    }
}

fn hover_cursor(cx: &Context, pos: Pos2) -> CursorIcon {
    let pointing = matches!(
        cx.inline_at(pos),
        Some((_, InlineItem { inline: Inline::Checkbox { .. }, .. }))
    ) || cx.gutter_arrow_at(pos).is_some();
    match pointing {
        true => CursorIcon::PointingHand,
        false => CursorIcon::Text,
    }
}
