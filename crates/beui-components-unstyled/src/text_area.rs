mod actions;
pub mod colors;
mod completion;
pub mod emoji;
pub mod hooks;
pub mod keys;
mod lines;
pub mod look;
pub mod rows;
pub mod state;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use accesskit::{Node, Role};

use text_editor_core::{
    CursorHorizontalPositionMetric, CursorLeftRightStop, DragSelectionMode, EditorCommand,
    ImeCommand, LRDirection, MoveMode, TextLanguage, UDDirection, VerticalMoveMode,
};

use beui_macros::{component, view};

use crate::{Scroll, ScrollbarStyle};
use beui_core::base::text::TextGeometry;
use beui_core::base::{ImeCursor, ItemSize, ScrollPosition};
use beui_core::clipboard::{PasteKind, Pasted};
use beui_core::color::Color32;
use beui_core::file_picker::PickedFile;
use beui_core::font::FontId;
use beui_core::geometry::{Pos2, Rect, Vec2};
use beui_core::input::{CursorIcon, ImeEvent, ImeText, Key, KeyPress, PointerPress};
use beui_core::layer::Dismiss;
use beui_core::node::NodeId;
use beui_core::rich::{CaretHandle, HANDLE_RADIUS, handle_center, toolbar_anchor};
use beui_view::primitives::*;
use beui_view::reactive::*;

use completion::{Completions, Query, query};
use lines::{Lines, SingleLine};
use rows::{BODY_SIZE, Composition, Inline, InlineItem, Row, galley, line_of};
use state::{AreaGeometry, Grab};
use text_editor_core::ListMarker;

pub use colors::{SyntaxColors, TextAreaColors};
pub use completion::{Completer, Completion, CompletionMenu, CompletionRowHandle};
pub use emoji::{emoji_completer, search_emoji};
pub use hooks::{
    LeafSpan, Margin, MarkerColumn, MarkerKind, PlainLine, QuoteBar, TableFill, TableLines,
    TextLeaf, TextLine, TextMarker,
};
pub use look::{BlockGeometry, ChipGeometry, TextAreaGeometry, TextAreaLook, WidgetGeometry};
pub use rows::{Prose, RowTable, TextWidget};
pub use state::{ProseLine, TextAreaLayout, TextAreaState};

const TOUCH_HANDLE_HIT_RADIUS: f32 = 24.0;
const CARET_HANDLE_TAP_SLACK: f32 = 4.0;
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

    fn rect(&self, placements: &Placements) -> Option<Rect> {
        placements.rect(self.row.try_get()?)
    }

    fn text(&self) -> Option<TextGeometry> {
        self.geometry.borrow().clone()
    }
}

struct Held {
    rows: RefCell<BTreeMap<usize, RowEntry>>,
    registered: Cell<u64>,
    placements: Placements,
    reveal_attempts: Cell<u8>,
    row_cache: RefCell<lines::RowCache>,
}

#[derive(Clone, Copy)]
struct Surface {
    state: TextAreaState,
    single_line: bool,
    line_numbers: bool,
    held: Stored<Held>,
    starts: Memo<Rc<Vec<usize>>>,
    visible: Memo<Vec<usize>>,
    widgets: Memo<Vec<TextWidget>>,
    client_colors: Memo<Vec<(u64, Color32)>>,
    colors: Memo<TextAreaColors>,
    geometry: Memo<TextAreaGeometry>,
    composition: Memo<Option<Composition>>,
    placeholder: Memo<String>,
    font_size: Memo<f32>,
    prose: Memo<Option<Prose>>,
    metrics: Memo<(f32, u64)>,
    gutter: Memo<f32>,
    padding: Memo<Vec2>,
    scroll: ReadSignal<ScrollPosition>,
    offset: Signal<f32>,
    shift: Signal<f32>,
    view_width: Memo<f32>,
    view_height: Memo<f32>,
    focused: Signal<bool>,
    keyboard: ReadSignal<bool>,
    autoscroll: Signal<bool>,
    viewport: NodeRef,
    list: NodeRef,
    anchor: NodeId,
    masked: Memo<bool>,
    disabled: Memo<bool>,
    remote_cursors: Memo<Vec<RemoteTextCursor>>,
    drop_caret: Memo<Option<usize>>,
    text_extent: Signal<Vec2>,
    block: Option<RenderFn<usize>>,
    selected_widget: Option<RenderFn<usize>>,
    leaf: RenderFn<TextLeaf, beui_view::components::text::Span>,
    marker: RenderFn<TextMarker>,
    line: RenderFn<TextLine>,
    rights: Signal<BTreeMap<u64, f32>>,
    on_content_size: Callback<Vec2>,
    completion: Memo<Option<Query>>,
    completions: Memo<Vec<Completion>>,
    completing: Memo<bool>,
    highlighted: Signal<usize>,
    dismissed: Signal<Option<usize>>,
    completion_layer: NodeRef,
    on_widget_press: Callback<usize, bool>,
    on_toolbar: Callback<Option<Rect>>,
    on_submit: ClickCallback,
}

type Context = Surface;

#[derive(Clone, Copy)]
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
                    Pos2::new(self.padding.get_untracked().x, y),
                    Vec2::new(self.caret_width(), self.row_estimate()),
                ))
            }
        }
    }

    fn index_at(&self, pos: Pos2) -> Option<usize> {
        self.hit(pos)
    }
}

impl Surface {
    fn caret_width(&self) -> f32 {
        self.geometry.get_untracked().caret_width
    }

    fn canvas_rect(&self) -> Option<Rect> {
        self.held
            .value()
            .placements
            .rect(self.state.canvas().try_get()?)
    }

    fn node_rect(&self, node: &NodeRef) -> Option<Rect> {
        self.held.value().placements.rect(node.try_get()?)
    }

    fn row_estimate(&self) -> f32 {
        let body = galley("", FontId::proportional(self.font_size.get_untracked()))
            .map_or(BODY_SIZE * 1.15, |galley| galley.line_height());
        let padding = match (self.single_line, self.prose.get_untracked()) {
            (true, _) => (0.0, 0.0),
            (false, Some(prose)) => prose.line_padding,
            (false, None) => self.geometry.get_untracked().line_padding,
        };
        body + padding.0 + padding.1
    }

    fn line_of(&self, byte: usize) -> usize {
        line_of(&self.starts.get_untracked(), byte)
    }

    fn entry(&self, line: usize) -> Option<RowEntry> {
        self.held.value().rows.borrow().get(&line).cloned()
    }

    fn caret_rect_at(&self, byte: usize) -> Option<Rect> {
        let entry = self.entry(self.line_of(byte))?;
        let display = entry.row().to_display(byte);
        entry.text()?.caret_rect(display, self.caret_width())
    }

    fn estimated_y(&self, byte: usize) -> Option<f32> {
        let line = self.line_of(byte);
        let list = self.list.try_get()?;
        let canvas = self.canvas_rect()?;
        let rect = self.held.value().placements.rect(list)?;
        let offset = list.row_offset(&line)?;
        Some(rect.min.y - canvas.min.y + offset)
    }

    fn row_at(&self, pos: Pos2) -> Option<RowEntry> {
        let held = self.held.value();
        let rows = held.rows.borrow();
        let placed: Vec<(Rect, &RowEntry)> = rows
            .values()
            .filter_map(|entry| Some((entry.rect(&held.placements)?, entry)))
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

    fn below_end(&self, pos: Pos2) -> bool {
        let held = self.held.value();
        let rows = held.rows.borrow();
        rows.values()
            .filter_map(|entry| entry.rect(&held.placements))
            .map(|rect| rect.max.y)
            .reduce(f32::max)
            .is_some_and(|bottom| pos.y > bottom)
    }

    fn hit(&self, pos: Pos2) -> Option<usize> {
        let entry = self.row_at(pos)?;
        let text = entry.text()?;
        let display = text.index_at(self.shifted(&text, pos))?;
        Some(entry.row().to_source(display))
    }

    fn inline_at(&self, pos: Pos2) -> Option<(RowEntry, usize, InlineItem)> {
        let entry = self.row_at(pos)?;
        let text = entry.text()?;
        let flow = text.inline_at(self.shifted(&text, pos))?;
        let row = entry.row();
        let index = row.flow_item(flow)?;
        let item = row.inline.get(index)?.clone();
        Some((entry, index, item))
    }

    fn gutter_arrow_at(&self, pos: Pos2) -> Option<usize> {
        let gutter = self.gutter.get_untracked();
        let entry = self.row_at(pos)?;
        let rect = entry.rect(&self.held.value().placements)?;
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
        let geometry = self.geometry.get_untracked();
        let size = geometry.gutter_arrow;
        let arrow = Rect::from_min_size(
            Pos2::new(
                rect.min.x + geometry.gutter_padding.0,
                rect.min.y + (row.line_height - size) / 2.0,
            ),
            Vec2::splat(size),
        );
        (collapsible && arrow.contains(pos)).then_some(start)
    }

    fn block_at(&self, pos: Pos2) -> Option<usize> {
        let held = self.held.value();
        let rows = held.rows.borrow();
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
            self.held.value().reveal_attempts.set(0);
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
        let mut offset = position.clamped();
        if rect.min.y - REVEAL_MARGIN.y < offset {
            offset = rect.min.y - REVEAL_MARGIN.y;
        }
        if rect.max.y + REVEAL_MARGIN.y > offset + position.viewport {
            offset = rect.max.y + REVEAL_MARGIN.y - position.viewport;
        }
        self.offset
            .set_unconditionally(offset.clamp(0.0, position.max_offset().max(offset)));
        let attempts = self.held.value().reveal_attempts.get();
        if !exact && attempts < REVEAL_ATTEMPTS {
            self.held.value().reveal_attempts.set(attempts + 1);
            self.state.reveal_cursor();
        } else {
            self.held.value().reveal_attempts.set(0);
        }
    }

    fn reveal_rect(&self, rect: Rect) {
        let position = self.scroll.get_untracked();
        let middle = rect.center().y - position.viewport / 2.0;
        self.offset
            .set_unconditionally(middle.clamp(0.0, position.max_offset()));
    }

    fn completion_key(&self, press: KeyPress) -> bool {
        if !self.completing.get_untracked() {
            return false;
        }
        let count = self.completions.get_untracked().len();
        let consumed = matches!(
            press.key,
            Key::ArrowDown | Key::ArrowUp | Key::Enter | Key::Tab | Key::Escape
        );
        if !consumed || !press.pressed || count == 0 {
            return consumed;
        }
        let highlighted = self.highlighted.get_untracked().min(count - 1);
        match press.key {
            Key::ArrowDown => self.highlighted.set((highlighted + 1) % count),
            Key::ArrowUp => self.highlighted.set((highlighted + count - 1) % count),
            Key::Enter | Key::Tab => self.complete(highlighted),
            _ => match self.completion_layer.try_get() {
                Some(layer) => {
                    layer.dismiss_overlay(Dismiss::Escape);
                }
                None => self.dismiss(),
            },
        }
        true
    }

    fn dismiss(&self) {
        let start = self
            .completion
            .get_untracked()
            .map(|query| query.range.start);
        self.dismissed.set(start);
    }

    fn complete(&self, index: usize) {
        let Some(query) = self.completion.get_untracked() else {
            return;
        };
        let Some(item) = self.completions.get_untracked().get(index).cloned() else {
            return;
        };
        let (anchor, focus) = self.state.with_core(|core| {
            (
                core.position(query.range.start),
                core.position(query.range.end),
            )
        });
        self.state
            .execute(EditorCommand::SetSelection { anchor, focus });
        self.state
            .execute(EditorCommand::InsertText(item.insert.as_bytes()));
        self.state.reveal_cursor();
        self.state.focus();
    }

    fn caret_x(&self, byte: usize) -> Option<f32> {
        let entry = self.entry(self.line_of(byte))?;
        let text = entry.text()?;
        let caret = text.caret_rect(entry.row().to_display(byte), self.caret_width())?;
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
        self.shift.set(shift.clamp(0.0, (width - room).max(0.0)));
    }

    fn text_size(&self) -> Vec2 {
        self.text_extent.get_untracked()
    }

    fn room(&self) -> f32 {
        (self.view_width.get_untracked()
            - self.padding.get_untracked().x * 2.0
            - self.caret_width())
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
        let state = self.state;
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
            true => (viewport.min.x - HANDLE_SLACK..=viewport.max.x + HANDLE_SLACK)
                .contains(&caret.min.x),
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

    fn toolbar_rect(&self) -> Option<Rect> {
        let carets = match (self.caret_handle(), self.selection_handles()) {
            (Some(caret), _) => self.caret_rect_at(caret)?,
            (None, Some(range)) => self
                .caret_rect_at(range.start)?
                .union(self.caret_rect_at(range.end)?),
            (None, None) => return None,
        };
        Some(toolbar_anchor(carets))
    }

    fn show_toolbar(&self) {
        self.on_toolbar.call(self.toolbar_rect());
    }

    fn on_caret_handle(&self, pos: Pos2) -> bool {
        let Some(caret) = self
            .caret_handle()
            .and_then(|byte| self.caret_rect_at(byte))
        else {
            return false;
        };
        let center = handle_center(caret, CaretHandle::Middle);
        (Vec2::new(pos.x, pos.y) - center).length() <= HANDLE_RADIUS + CARET_HANDLE_TAP_SLACK
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
        self.autoscroll.set(beyond.is_some());
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

#[derive(Clone, Copy)]
enum Beyond {
    Before,
    After,
}

const ACCESSIBLE_TEXT_LIMIT: usize = 64 * 1024;

fn accessible_range(bytes: &[u8], caret: usize) -> Range<usize> {
    if bytes.len() <= ACCESSIBLE_TEXT_LIMIT {
        return 0..bytes.len();
    }
    let from = caret
        .min(bytes.len())
        .saturating_sub(ACCESSIBLE_TEXT_LIMIT / 2);
    let start = bytes[..from]
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(0, |newline| newline + 1);
    let to = (start + ACCESSIBLE_TEXT_LIMIT).min(bytes.len());
    let end = bytes[to..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map_or(bytes.len(), |newline| to + newline);
    start..end
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
            (
                Grab::Selection(cx.state.with_core(|core| core.position(fixed))),
                moving,
            )
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
    let position = cx.state.with_core(|core| core.position(target));
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
    cx.focused.set(true);
    cx.state.set_touch_mode(press.touch);
    cx.state.end_grab();
    if press.touch
        && let Some(handle) = cx.handle_at(press.pos)
    {
        begin_handle_drag(cx, handle, press.pos);
        cx.on_toolbar.call(None);
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
        let position = cx.state.with_core(|core| core.position(line_start));
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
        Some(Grab::Caret) if cx.on_caret_handle(press.pos) => {
            cx.show_toolbar();
            return;
        }
        Some(Grab::Caret) => {}
        Some(Grab::Selection(_)) => {
            cx.show_toolbar();
            return;
        }
        None => {}
    }
    let Some(target) = cx.hit(press.pos) else {
        return;
    };
    if cx.state.selection_contains(target) {
        cx.show_toolbar();
        cx.state.set_selecting(false);
        return;
    }
    cx.state.set_caret_handle(true);
    select_at(cx, press.pos, press.clicks, false, false);
    cx.state.set_selecting(false);
    match press.clicks >= WORD_CLICKS {
        true => cx.show_toolbar(),
        false => cx.on_toolbar.call(None),
    }
}

fn hold(cx: &Context, press: PointerPress) {
    if !press.touch || cx.disabled.get_untracked() || cx.state.grab().is_some() {
        return;
    }
    let inside = cx
        .hit(press.pos)
        .is_some_and(|target| cx.state.selection_contains(target));
    if !inside {
        cx.state.set_caret_handle(true);
        select_at(cx, press.pos, WORD_CLICKS, false, false);
        cx.state.set_selecting(false);
    }
    cx.show_toolbar();
}

fn select_at(cx: &Context, pos: Pos2, clicks: u32, extend: bool, syntax: bool) {
    if let Some((entry, index, item)) = cx.inline_at(pos) {
        match item.inline {
            Inline::Marker(ListMarker::Check(_)) => {
                cx.state.set_selecting(false);
                return;
            }
            Inline::Widget(index) => {
                let Some(widget) = cx.widgets.get_untracked().get(index).cloned() else {
                    return;
                };
                let (anchor, focus) = cx.state.with_core(|core| {
                    (
                        core.position(widget.range.start),
                        core.position(widget.range.end),
                    )
                });
                cx.state
                    .execute(EditorCommand::SetSelection { anchor, focus });
                cx.state.set_selecting(false);
                return;
            }
            Inline::Void => {
                let Some(range) = entry.row().inline_source(index) else {
                    return;
                };
                let (anchor, focus) = cx
                    .state
                    .with_core(|core| (core.position(range.start), core.position(range.end)));
                cx.state
                    .execute(EditorCommand::SetSelection { anchor, focus });
                cx.state.set_selecting(false);
                return;
            }
            Inline::Run | Inline::Marker(_) | Inline::Quote => {}
        }
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
    if clicks == 1 && !extend && cx.below_end(pos) {
        let end = cx.state.with_core(|core| core.position(usize::MAX));
        cx.state.execute(EditorCommand::SetSelection {
            anchor: end,
            focus: end,
        });
        cx.state.execute(EditorCommand::MoveCursorUpDown {
            direction: UDDirection::Down,
            mode: VerticalMoveMode::Move,
            metric: CursorHorizontalPositionMetric::Byte,
            stop: CursorLeftRightStop::UnicodeGraphemeCluster,
        });
        let Some(&caret) = cx.state.caret_indices().first() else {
            return;
        };
        let position = cx.state.with_core(|core| core.position(caret));
        cx.state.execute(EditorCommand::Click {
            position,
            mode,
            extend: false,
            select_syntax_node: syntax,
        });
        cx.state.set_selecting(true);
        cx.state.reveal_cursor();
        return;
    }
    let position = cx.state.with_core(|core| core.position(target));
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
    let target = match cx.below_end(inside) {
        true => usize::MAX,
        false => {
            let Some(target) = cx.hit(inside) else {
                return;
            };
            target
        }
    };
    let position = cx.state.with_core(|core| core.position(target));
    cx.state.execute(EditorCommand::Drag(position));
    cx.step_beyond(beyond, MoveMode::Select);
    cx.state.reveal_cursor();
}

fn blur(cx: &Context) {
    cx.on_toolbar.call(None);
    cx.state
        .execute(EditorCommand::Ime(ImeCommand::FinishComposing));
    cx.state.end_grab();
    cx.state.set_selecting(false);
    cx.state.set_caret_handle(false);
    cx.autoscroll.set(false);
    cx.state.external_edit();
}

fn release(cx: &Context, active: bool) {
    if active {
        return;
    }
    if let Some(Grab::Selection(_)) = cx.state.end_grab() {
        cx.show_toolbar();
    }
    cx.state.set_selecting(false);
    cx.autoscroll.set(false);
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

fn ime(cx: &Context, event: ImeEvent, key: impl Fn(KeyPress) -> bool) {
    if cx.disabled.get_untracked() {
        return;
    }
    let typed = |text: &str| -> String {
        text.chars()
            .filter(|letter| !letter.is_control() || (!cx.single_line && *letter == '\t'))
            .collect()
    };
    let command = match &event {
        ImeEvent::Enabled => return,
        ImeEvent::Disabled | ImeEvent::FinishComposing => ImeCommand::FinishComposing,
        ImeEvent::SetComposingText(text) => {
            let text = typed(text);
            cx.state
                .execute(EditorCommand::Ime(ImeCommand::SetComposingText(&text)));
            cx.state.set_caret_handle(false);
            cx.state.reveal_cursor();
            return;
        }
        ImeEvent::CommitText(text) => {
            let mut lines = text.split('\n').peekable();
            while let Some(line) = lines.next() {
                let line = typed(line);
                cx.state
                    .execute(EditorCommand::Ime(ImeCommand::CommitText(&line)));
                if lines.peek().is_some() {
                    for pressed in [true, false] {
                        key(KeyPress {
                            key: Key::Enter,
                            pressed,
                            repeat: false,
                            modifiers: Default::default(),
                        });
                    }
                }
            }
            cx.state.set_caret_handle(false);
            cx.state.reveal_cursor();
            return;
        }
        ImeEvent::SetComposingRegion(range) => ImeCommand::SetComposingRegion(range.clone()),
        ImeEvent::ReplaceText { range, text } => {
            let text = typed(text);
            cx.state
                .execute(EditorCommand::Ime(ImeCommand::ReplaceText {
                    range: range.clone(),
                    text: &text,
                }));
            cx.state.set_caret_handle(false);
            cx.state.reveal_cursor();
            return;
        }
        ImeEvent::DeleteSurrounding { before, after } => ImeCommand::DeleteSurrounding {
            before: *before,
            after: *after,
        },
        ImeEvent::SetSelection { anchor, focus } => ImeCommand::SetSelection {
            anchor: *anchor,
            focus: *focus,
        },
    };
    cx.state.execute(EditorCommand::Ime(command));
    cx.state.reveal_cursor();
}

fn field_key(cx: &Context, press: KeyPress) -> bool {
    !cx.disabled.get_untracked()
        && !keys::leaves_on_tab(cx, press)
        && (cx.completion_key(press) || keys::key(cx, press))
}

fn gutter_width(line_count: usize, font_size: f32, geometry: &TextAreaGeometry) -> f32 {
    let digit =
        galley("0", FontId::monospace(font_size)).map_or(font_size * 0.6, |galley| galley.size().x);
    let mut digits = 1;
    let mut value = line_count.max(1);
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    let (left, right) = geometry.gutter_padding;
    left + geometry.gutter_arrow + digit * digits as f32 + right
}

#[derive(Clone, Copy)]
struct Field {
    cx: Context,
    described: Memo<Node>,
    frame: Stored<RefCell<Option<Render<Child>>>>,
    inner: NodeRef,
    field_placed: Signal<Option<ReadSignal<Rect>>>,
    cursor: Memo<CursorIcon>,
    hover_cursor: Signal<CursorIcon>,
    autoscroll: ReadSignal<bool>,
    tab_stop: Memo<bool>,
    keyboard_on_focus: bool,
    ime_cursor: Memo<Option<ImeCursor>>,
    ime_text: Memo<Option<ImeText>>,
    on_focus_change: Callback<bool>,
    on_hover_change: Callback<bool>,
    pastes_images: bool,
    on_image_paste: Callback<PickedFile>,
}

#[component]
pub fn TextArea(
    #[prop(default = ComponentRef::new())] api: ComponentRef<TextAreaApi>,
    state: TextAreaState,
    #[prop(default = false)] single_line: bool,
    #[prop(default = true)] line_numbers: bool,
    #[prop(default = Vec::new())] widgets: Prop<Vec<TextWidget>>,
    colors: Prop<TextAreaColors>,
    look: TextAreaLook,
    #[prop(default = Vec::new())] remote_cursors: Prop<Vec<RemoteTextCursor>>,
    #[prop(default = None)] drop_caret: Prop<Option<usize>>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] password: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    #[prop(default = true)] keyboard_on_focus: bool,
    #[prop(default = false)] disabled: Prop<bool>,
    font_size: Prop<f32>,
    padding: Prop<Vec2>,
    #[prop(default = None)] prose: Prop<Option<Prose>>,
    #[prop(default = false)] show_runs: Prop<bool>,
    #[prop(default = Vec::new())] client_colors: Prop<Vec<(u64, Color32)>>,
    #[prop(default = ScrollbarStyle::default())] scrollbar: ScrollbarStyle,
    accessibility: Option<Prop<Node>>,
    frame: Option<Render<Child>>,
    block: Option<RenderFn<usize>>,
    selected_widget: Option<RenderFn<usize>>,
    on_content_size: Callback<Vec2>,
    #[prop(default = Completer::none())] completer: Completer,
    completion_menu: Option<CompletionMenu>,
    on_widget_press: Callback<usize, bool>,
    on_toolbar: Callback<Option<Rect>>,
    on_submit: ClickCallback,
    on_focus_change: Callback<bool>,
    on_hover_change: Callback<bool>,
    #[prop(default = false)] pastes_images: bool,
    on_image_paste: Callback<PickedFile>,
) -> NodeId {
    let focus_request = focused;
    let search = completer.search.clone();
    let trigger = completer.trigger;
    view! {
        <LayoutBuilder>|size: ReadSignal<Vec2>|
            <PlacementBuilder>|area: ReadSignal<Rect>| {
                let field_placed =
                    create_signal(None::<ReadSignal<Rect>>);
                let placed = create_memo(move || match single_line {
                    true => field_placed.get().map_or(Rect::ZERO, |placed| placed.get()),
                    false => area.get(),
                });
                let scale = pixels_per_point();
                let fonts = use_font_revision();
                let scroll = create_signal(ScrollPosition::ZERO);
                let offset = create_signal(0.0_f32);
                let shift = create_signal(0.0_f32);
                let focused = create_signal(false);
                let autoscroll = create_signal(false);
                let viewport = NodeRef::new();
                let (outer, inner) = match single_line {
                    true => (NodeRef::new(), viewport),
                    false => (viewport, NodeRef::new()),
                };
                let masked = create_memo(move || password.get());
                let placeholder = create_memo(move || placeholder.get());
                let disabled = create_memo(move || disabled.get());
                let font_size = create_memo(move || font_size.get());
                let prose = create_memo(move || prose.get());
                let padding = create_memo(move || padding.get());
                let widgets = create_memo(move || widgets.get());
                let colors = create_memo(move || colors.get());
                let geometry = look.geometry;
                let geometry = create_memo(move || geometry.get());
                let remote_cursors = create_memo(move || remote_cursors.get());
                let drop_caret = create_memo(move || drop_caret.get());
                let text_extent = create_signal(Vec2::ZERO);
                let view_size = create_memo(move || match single_line {
                    true => placed.get().size(),
                    false => size.get(),
                });
                let view_width = create_memo(move || view_size.get().x);
                let client_colors = create_memo(move || client_colors.get());
                create_effect(move || {
                    let shown = show_runs.get();
                    untrack(|| state.set_show_runs(shown));
                });

                let role = match single_line {
                    true => Role::TextInput,
                    false => Role::MultilineTextInput,
                };
                let accessibility = accessibility.unwrap_or_else(|| Prop::Static(Node::new(role)));
                let accessible = state.content();
                let moved = state.cursors();
                let described = create_memo(
                    move || {
                        accessible.get();
                        let bytes = state.bytes();
                        if bytes.len() > ACCESSIBLE_TEXT_LIMIT {
                            moved.get();
                        }
                        let caret = state.caret_indices().first().copied().unwrap_or(0);
                        let mut node = accessibility.get();
                        let value = String::from_utf8_lossy(&bytes[accessible_range(&bytes, caret)]).into_owned();
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
                    },
                );

                let content = state.content();
                let starts = create_memo(move || {
                    content.get();
                    state.with_snapshot(|snapshot| match snapshot.starts.is_empty() {
                        true => Rc::new(vec![0]),
                        false => Rc::clone(&snapshot.starts),
                    })
                });
                let visible = create_memo(move || {
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
                });
                let gutter = create_memo(move || {
                    scale.get();
                    fonts.get();
                    content.get();
                    let rich = state.with_snapshot(|snapshot| snapshot.language == TextLanguage::RichText);
                    match single_line || !line_numbers || rich {
                        true => 0.0,
                        false => gutter_width(starts.get().len(), font_size.get(), &geometry.get()),
                    }
                });
                let composition = create_memo(move || {
                    state.cursors().get();
                    state.content().get();
                    Some(Composition {
                        range: state.with_core(|core| core.composition())?,
                    })
                });
                let completion = create_memo(move || {
                    let trigger = trigger?;
                    state.cursors().get();
                    state.content().get();
                    if !focused.get() || state.with_core(|core| core.composition()).is_some() {
                        return None;
                    }
                    query(state, trigger)
                });
                let completions = create_memo(move || {
                    match (&completion.get(), &search) {
                        (Some(query), Some(search)) => search(&query.text),
                        _ => Vec::new(),
                    }
                });
                let dismissed = create_signal(None::<usize>);
                let completing = create_memo(move || {
                    let Some(query) = completion.get() else {
                        return false;
                    };
                    dismissed.get() != Some(query.range.start) && !completions.get().is_empty()
                });
                let highlighted = create_signal(0_usize);
                create_effect(move || {
                    match completion.get() {
                        Some(_) => highlighted.set(0),
                        None => dismissed.set(None),
                    }
                });
                let caret_width = create_memo(move || Extent::px(geometry.get().caret_width));
                let anchor = view! {
                    <Frame width=caret_width />
                };

                let cx = Surface {
                    state,
                    single_line,
                    line_numbers,
                    held: store(Held {
                        rows: RefCell::new(BTreeMap::new()),
                        registered: Cell::new(0),
                        placements: placements(),
                        reveal_attempts: Cell::new(0),
                        row_cache: RefCell::default(),
                    }),
                    starts,
                    visible,
                    widgets,
                    client_colors,
                    colors,
                    geometry,
                    composition,
                    placeholder,
                    font_size,
                    prose,
                    metrics: create_memo(move || (scale.get(), fonts.get())),
                    gutter,
                    padding,
                    scroll: scroll.read_only(),
                    offset,
                    shift,
                    view_width,
                    view_height: create_memo(move || view_size.get().y),
                    focused,
                    keyboard: use_keyboard(),
                    autoscroll,
                    viewport,
                    list: NodeRef::new(),
                    anchor,
                    masked,
                    disabled,
                    remote_cursors,
                    drop_caret,
                    text_extent,
                    block,
                    selected_widget,
                    leaf: look.leaf,
                    marker: look.marker,
                    line: look.line,
                    rights: create_signal(BTreeMap::new()),
                    on_content_size,
                    completion,
                    completions,
                    completing,
                    highlighted,
                    dismissed,
                    completion_layer: NodeRef::new(),
                    on_widget_press,
                    on_toolbar,
                    on_submit,
                };
                state.publish_layout(TextAreaLayout::new(Rc::new(cx)));

                if single_line {
                    create_effect(move || {
                        let width = text_extent.get().x;
                        view_width.get();
                        let room = untrack(|| cx.room());
                        let most = (width - room).max(0.0);
                        if shift.get_untracked() > most {
                            shift.set(most);
                        }
                    });
                }

                let ime_cursor = create_memo(move || {
                    state.cursors().get();
                    if !focused.get() {
                        return None;
                    }
                    Some(ImeCursor {
                        node: anchor,
                        rect: None,
                    })
                });
                let ime_text = create_memo(move || {
                    state.cursors().get();
                    state.content().get();
                    if !focused.get() {
                        return None;
                    }
                    let surrounding = state.with_core(|core| core.ime_state())?;
                    Some(ImeText {
                        start: surrounding.start,
                        text: surrounding.text,
                        selection: surrounding.selection,
                        composing: surrounding.composing,
                    })
                });

                let focus_requests = state.focus_requests();
                create_effect(move || {
                    if focus_requests.get() > 0 {
                        focused.set(true);
                    }
                });
                create_effect(move || focused.set(focus_request.get()));
                let content = state.content();
                let shown = create_memo(move || {
                    content.get();
                    let value = String::from_utf8_lossy(&state.bytes()).into_owned();
                    match (value.is_empty(), masked.get()) {
                        (true, _) => placeholder.get(),
                        (false, true) => rows::mask(&value),
                        (false, false) => value,
                    }
                });
                api.fill(TextAreaApi(Parts {
                    cx,
                    shown,
                }));
                let revealing = create_timer(move || {
                    cx.reveal_caret();
                    None
                });
                let reveals = state.reveals();
                create_effect(move || {
                    reveals.get();
                    revealing.start(Duration::ZERO);
                });
                let hover_cursor = create_signal(CursorIcon::Text);
                let cursor = create_memo(move || match disabled.get() {
                    true => CursorIcon::Default,
                    false => hover_cursor.get(),
                });
                let tab_stop = create_memo(move || !disabled.get());
                let field = Field {
                    described,
                    frame: store(RefCell::new(frame)),
                    inner,
                    field_placed,
                    cx,
                    cursor,
                    hover_cursor,
                    autoscroll: autoscroll.read_only(),
                    tab_stop,
                    keyboard_on_focus,
                    ime_cursor,
                    ime_text,
                    on_focus_change,
                    on_hover_change,
                    pastes_images,
                    on_image_paste,
                };
                let multi_line = !single_line;
                let width = create_memo(move || size.get().x);
                let height = create_memo(move || size.get().y);
                let border_color = create_memo(move || colors.get().gutter_border);
                let border_x = create_memo(move || gutter.get() - 1.0);
                let border_width = create_memo(move || match gutter.get() > 0.0 {
                    true => 1.0,
                    false => 0.0,
                });
                let has_menu = completion_menu.is_some();
                let menu = create_memo(move || has_menu && completing.get());
                if multi_line {
                    action_scope(&outer);
                    actions::register(state, disabled);
                }

                view! {
                    <Frame @node_ref=&outer>
                        <List spacing=0.0>
                            <Show condition={single_line}>
                                <Editing @sizing=ItemSize::Percent(100.0) field={field} />
                            </Show>
                            <Show condition={multi_line}>||
                                <Canvas
                                    @sizing=ItemSize::Percent(100.0)
                                    width={width}
                                    height={height}
                                >
                                    <CanvasItem
                                        x={border_x}
                                        y=0.0
                                        width={border_width}
                                        height={height}
                                    >
                                        <Frame fill=border_color />
                                    </CanvasItem>
                                    <CanvasItem x=0.0 y=0.0 width={width} height={height}>
                                        <Scroll offset={offset} scrollbar on_change={scroll}>
                                            <Editing field={field} />
                                        </Scroll>
                                    </CanvasItem>
                                </Canvas>
                            </Show>
                            <Show condition={menu}>|| {
                                let menu = completion_menu
                                    .expect("a completion menu is only shown when the area was given one");
                                view! {
                                    <Completions cx menu />
                                }
                            }</Show>
                        </List>
                    </Frame>
                }
            }</PlacementBuilder>
        </LayoutBuilder>
    }
}

#[component]
fn Editing(field: Field) -> NodeId {
    let Field {
        cx,
        described,
        frame,
        inner,
        field_placed,
        cursor,
        hover_cursor: pointer_cursor,
        autoscroll,
        tab_stop,
        keyboard_on_focus,
        ime_cursor,
        ime_text,
        on_focus_change,
        on_hover_change,
        pastes_images,
        on_image_paste,
    } = field;
    let pastes = match pastes_images {
        true => vec![PasteKind::Image, PasteKind::Text],
        false => vec![PasteKind::Text],
    };
    let focused = cx.focused;
    component_accessibility(described);
    let single_line = cx.single_line;
    let disabled = cx.disabled;
    view! {
        <Interactive
            focusable=true
            focused
            tab_stop
            ime={create_memo(move || !disabled.get())}
            keyboard_on_focus
            ime_cursor
            ime_text
            on_focus_change={move |is_focused: bool| {
                focused.set(is_focused);
                if !is_focused {
                    blur(&cx);
                }
                on_focus_change.call(is_focused);
            }}
            on_text={move |typed: String| insert_text(&cx, &typed)}
            pastes
            on_paste={move |pasted: Pasted| match pasted {
                Pasted::Text(typed) => insert_text(&cx, &typed),
                Pasted::Image(image) => on_image_paste.call(image),
            }}
            on_ime={move |event: ImeEvent| {
                ime(&cx, event, |press| field_key(&cx, press))
            }}
            on_key={move |press: KeyPress| {
                cx.state.with_core(|core| core.composition()).is_some()
                    || field_key(&cx, press)
            }}
            cursor
            repeat_drag={autoscroll}
            capture_presses={create_memo(move || !disabled.get())}
            claim_at={move |(pos, _): (Pos2, beui_core::input::Modifiers)| {
                !cx.disabled.get_untracked() && cx.handle_at(pos).is_some()
            }}
            on_press={move |event: PointerPress| press(&cx, event)}
            on_click_at={move |event: PointerPress| tap(&cx, event)}
            on_secondary_press={move |event: PointerPress| hold(&cx, event)}
            on_drag={move |event: PointerPress| extend(&cx, event)}
            on_active_change={move |active: bool| release(&cx, active)}
            on_hover_change={on_hover_change}
            on_hover_move={move |event: PointerPress| {
                pointer_cursor.set(hover_cursor(&cx, event.pos));
            }}
        >
            {{
                match single_line {
                    false => view! {
                        <Lines cx={cx} />
                    },
                    true => {
                        let field = view! {
                            <FieldFrame @node_ref=&inner report={field_placed}>
                                <SingleLine cx={cx} />
                            </FieldFrame>
                        };
                        match frame.value().borrow_mut().take() {
                            Some(frame) => frame.call(field),
                            None => field,
                        }
                    }
                }
            }}
        </Interactive>
    }
}

fn area_hit(tree: &impl Inspect, cx: &Context, pos: Pos2) -> Option<usize> {
    let held = cx.held.value();
    let rows = held.rows.borrow();
    let placed: Vec<(Rect, &RowEntry)> = rows
        .values()
        .filter_map(|entry| Some((tree.node_rect(entry.row.try_get()?)?, entry)))
        .collect();
    let (_, entry) = placed
        .iter()
        .find(|(rect, _)| pos.y < rect.max.y)
        .or(placed.last())?;
    let display = entry.text()?.index_at(pos)?;
    Some(entry.row().to_source(display))
}

#[component]
fn FieldFrame(report: Signal<Option<ReadSignal<Rect>>>, children: Child) -> NodeId {
    view! {
        <PlacementBuilder>|placed: ReadSignal<Rect>| {
            report.set_unconditionally(Some(placed));
            view! {
                <Frame>{children}</Frame>
            }
        }</PlacementBuilder>
    }
}

fn hover_cursor(cx: &Context, pos: Pos2) -> CursorIcon {
    let pointing = matches!(
        cx.inline_at(pos),
        Some((
            _,
            _,
            InlineItem {
                inline: Inline::Marker(ListMarker::Check(_)),
                ..
            }
        ))
    ) || cx.gutter_arrow_at(pos).is_some();
    match pointing {
        true => CursorIcon::PointingHand,
        false => CursorIcon::Text,
    }
}

#[derive(Clone, Copy)]
pub struct TextAreaApi(Parts);

impl TextAreaApi {
    pub fn shown(&self) -> String {
        self.0.shown.get_untracked()
    }

    pub fn index_at(&self, tree: &impl Inspect, pos: Pos2) -> usize {
        let cx = self.0.cx;
        area_hit(tree, &cx, pos).unwrap_or(0)
    }

    pub fn remote_cursors(&self) -> Vec<RemoteTextCursor> {
        self.0.cx.remote_cursors.get_untracked()
    }

    pub fn state(&self) -> TextAreaState {
        self.0.cx.state
    }

    pub fn handles(&self, tree: &impl Inspect) -> Vec<Pos2> {
        let cx = self.0.cx;
        let held = cx.held.value();
        let rows = held.rows.borrow();
        cx.handles()
            .into_iter()
            .filter_map(|(handle, byte)| {
                let line = line_of(&cx.starts.get_untracked(), byte);
                let entry = rows.get(&line)?;
                let caret = entry
                    .text()?
                    .caret_rect(entry.row().to_display(byte), cx.caret_width())?;
                let viewport = cx.viewport.try_get().and_then(|node| tree.node_rect(node));
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
}
