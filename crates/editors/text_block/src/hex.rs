use std::ops::Range;
use std::rc::Rc;

use beui::reactive::{
    ClickCatcher, Draw, Drawing, Focusable, Frame, Memo, NodeRef, Prop, clone, component,
    component_size, copy_text, create_memo, create_signal, each_frame, layout_text, request_paste,
    view, with_document,
};
use beui::unstyled::Scroll;
use beui::unstyled::TextAreaColors;
use beui::{
    Color32, CursorIcon, FontId, Key, KeyPress, NodeId, Page, PageShape, PointerPress, Pos2, Rect,
    TextLayout, Vec2,
};
use text_editor_core::{CursorLeftRightStop, EditorCommand, LRDirection};

use crate::app::state::Shared;

const BYTES_PER_ROW: usize = 16;
const GROUP_SIZE: usize = 8;
const TEXT_SIZE: f32 = 13.0;
const ROW_HEIGHT: f32 = TEXT_SIZE * 1.6;
const PADDING: Vec2 = Vec2::new(12.0, 8.0);
const PAGE_ROWS: usize = 16;
const COLORS: TextAreaColors = TextAreaColors::DEFAULT;

pub(crate) fn intrinsic_size(len: usize, width: f32) -> Vec2 {
    let rows = len.div_ceil(BYTES_PER_ROW).max(1);
    Vec2::new(width, rows as f32 * ROW_HEIGHT + PADDING.y * 2.0)
}

#[derive(Clone, PartialEq)]
pub(crate) struct HexGeometry {
    char_width: f32,
    hex_x: [f32; BYTES_PER_ROW],
    ascii_x: [f32; BYTES_PER_ROW],
    total_width: f32,
}

impl HexGeometry {
    fn new(char_width: f32) -> Self {
        let offset_end = char_width * 8.0;
        let hex_start = offset_end + char_width * 2.0;
        let mut hex_x = [0.0; BYTES_PER_ROW];
        for (col, x) in hex_x.iter_mut().enumerate() {
            let group_gap = if col >= GROUP_SIZE { char_width } else { 0.0 };
            *x = hex_start + col as f32 * char_width * 3.0 + group_gap;
        }
        let hex_end = hex_x[BYTES_PER_ROW - 1] + char_width * 2.0;
        let ascii_start = hex_end + char_width * 2.0;
        let mut ascii_x = [0.0; BYTES_PER_ROW];
        for (col, x) in ascii_x.iter_mut().enumerate() {
            *x = ascii_start + col as f32 * char_width;
        }
        let total_width = ascii_x[BYTES_PER_ROW - 1] + char_width;
        Self {
            char_width,
            hex_x,
            ascii_x,
            total_width,
        }
    }

    fn region_split(&self) -> f32 {
        let hex_end = self.hex_x[BYTES_PER_ROW - 1] + self.char_width * 2.0;
        (hex_end + self.ascii_x[0]) / 2.0
    }

    fn byte_at(&self, local: Vec2, rows: usize) -> usize {
        let row = ((local.y / ROW_HEIGHT).floor().max(0.0) as usize).min(rows.saturating_sub(1));
        let ascii_region = local.x >= self.region_split();
        let col = if ascii_region {
            nearest_column(&self.ascii_x, self.char_width * 0.5, local.x)
        } else {
            nearest_column(&self.hex_x, self.char_width, local.x)
        };
        row * BYTES_PER_ROW + col
    }
}

fn nearest_column(centers: &[f32; BYTES_PER_ROW], center_offset: f32, x: f32) -> usize {
    let mut best = 0;
    let mut best_distance = f32::MAX;
    for (col, left) in centers.iter().enumerate() {
        let distance = (x - (left + center_offset)).abs();
        if distance < best_distance {
            best_distance = distance;
            best = col;
        }
    }
    best
}

fn document_len(state: &Shared) -> usize {
    state.text.bytes().len()
}

fn cursor_range(state: &Shared) -> Range<usize> {
    let core = state.text.core();
    core.cursor_positions()
        .first()
        .copied()
        .and_then(|cursor| core.selection_range(&cursor))
        .unwrap_or(0..0)
}

fn select_byte(state: &Shared, index: usize) {
    let len = document_len(state);
    let index = index.min(len);
    let insert = state.hex_insert_mode.get_untracked();
    let focus_index = match !insert && index < len {
        true => index + 1,
        false => index,
    };
    let (anchor, focus) = {
        let core = state.text.core();
        (core.position(index), core.position(focus_index))
    };
    state
        .text
        .execute(EditorCommand::SetSelection { anchor, focus });
}

fn set_range(state: &Shared, anchor_byte: usize, target_byte: usize) {
    let len = document_len(state);
    let anchor_byte = anchor_byte.min(len);
    let target_byte = target_byte.min(len);
    let (anchor, focus) = {
        let core = state.text.core();
        match target_byte >= anchor_byte {
            true => (
                core.position(anchor_byte),
                core.position((target_byte + 1).min(len)),
            ),
            false => (
                core.position((anchor_byte + 1).min(len)),
                core.position(target_byte),
            ),
        }
    };
    state
        .text
        .execute(EditorCommand::SetSelection { anchor, focus });
}

fn commit_byte(state: &Shared, byte_index: usize, byte: u8) {
    let len = document_len(state);
    let insert = state.hex_insert_mode.get_untracked();
    let overwrite_end = match !insert && byte_index < len {
        true => byte_index + 1,
        false => byte_index.min(len),
    };
    let (anchor, focus) = {
        let core = state.text.core();
        (
            core.position(byte_index.min(len)),
            core.position(overwrite_end),
        )
    };
    state
        .text
        .execute(EditorCommand::SetSelection { anchor, focus });
    state.text.execute(EditorCommand::InsertText(&[byte]));
    let next = byte_index + 1;
    state.hex_selection_anchor.set(Some(next));
    if !insert {
        select_byte(state, next);
    }
}

fn type_nibble(state: &Shared, digit: u8) {
    let byte_index = cursor_range(state).start;
    match state.hex_pending_nibble.take() {
        None => state.hex_pending_nibble.set(Some(digit)),
        Some(high) => commit_byte(state, byte_index, (high << 4) | digit),
    }
}

pub(crate) fn type_text(state: &Shared, text: &str) {
    for character in text.chars() {
        if let Some(digit) = character.to_digit(16) {
            type_nibble(state, digit as u8);
        }
    }
}

fn delete(state: &Shared, direction: LRDirection) {
    state.hex_pending_nibble.set(None);
    state.text.execute(EditorCommand::Delete {
        direction,
        stop: CursorLeftRightStop::Byte,
    });
    let index = cursor_range(state).start;
    state.hex_selection_anchor.set(Some(index));
    if !state.hex_insert_mode.get_untracked() {
        select_byte(state, index);
    }
}

fn copy(state: &Shared, cut: bool) {
    let range = cursor_range(state);
    if range.is_empty() {
        return;
    }
    let bytes = state
        .text
        .bytes()
        .get(range.clone())
        .map(<[u8]>::to_vec)
        .unwrap_or_default();
    if bytes.is_empty() {
        return;
    }
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ");
    copy_text(hex);
    if cut {
        delete(state, LRDirection::Left);
    }
}

fn navigate(state: &Shared, press: KeyPress) -> bool {
    let len = document_len(state);
    let current = cursor_range(state).start;
    let modifiers = press.modifiers;
    let target = match press.key {
        Key::ArrowLeft => current.saturating_sub(1),
        Key::ArrowRight => (current + 1).min(len),
        Key::ArrowUp => current.saturating_sub(BYTES_PER_ROW),
        Key::ArrowDown => (current + BYTES_PER_ROW).min(len),
        Key::Home if modifiers.ctrl => 0,
        Key::Home => current - current % BYTES_PER_ROW,
        Key::End if modifiers.ctrl => len,
        Key::End => (current - current % BYTES_PER_ROW + BYTES_PER_ROW - 1).min(len),
        Key::PageUp => current.saturating_sub(BYTES_PER_ROW * PAGE_ROWS),
        Key::PageDown => (current + BYTES_PER_ROW * PAGE_ROWS).min(len),
        _ => return false,
    };
    state.hex_pending_nibble.set(None);
    if modifiers.shift {
        let anchor = state.hex_selection_anchor.get().unwrap_or(current);
        state.hex_selection_anchor.set(Some(anchor));
        set_range(state, anchor, target);
    } else {
        state.hex_selection_anchor.set(Some(target));
        select_byte(state, target);
    }
    true
}

fn key(state: &Shared, press: KeyPress) -> bool {
    if !press.pressed {
        return false;
    }
    let modifiers = press.modifiers;
    match press.key {
        Key::C if modifiers.ctrl => copy(state, false),
        Key::X if modifiers.ctrl => copy(state, true),
        Key::V if modifiers.ctrl => request_paste(),
        Key::Backspace => delete(state, LRDirection::Left),
        Key::Delete => delete(state, LRDirection::Right),
        Key::A if modifiers.ctrl => {
            state.hex_pending_nibble.set(None);
            state.text.execute(EditorCommand::SelectAll);
        }
        Key::Z if modifiers.ctrl => {
            state.hex_pending_nibble.set(None);
            state.text.execute(match modifiers.shift {
                true => EditorCommand::Redo,
                false => EditorCommand::Undo,
            });
        }
        Key::Y if modifiers.ctrl => {
            state.hex_pending_nibble.set(None);
            state.text.execute(EditorCommand::Redo);
        }
        _ => return navigate(state, press),
    }
    true
}

fn page(state: &Shared, geometry: &HexGeometry, size: Vec2) -> Page {
    let bytes = state.text.bytes();
    let len = bytes.len();
    let rows = len.div_ceil(BYTES_PER_ROW).max(1);
    let selected = cursor_range(state);
    let focus = {
        let core = state.text.core();
        core.cursor_positions()
            .first()
            .copied()
            .and_then(|cursor| core.position_index(cursor.pos.focus))
    };
    let font = FontId::monospace(TEXT_SIZE);
    let mut shapes = vec![PageShape::Rect {
        rect: Rect::from_min_size(Pos2::ZERO, size),
        corner_radius: 0.0,
        color: COLORS.surface,
    }];
    let push_text = |origin: Pos2, string: &str, color: Color32| {
        layout_text(string, font, TextLayout::DEFAULT).map(|galley| PageShape::Text {
            origin,
            galley,
            color,
        })
    };
    for row in 0..rows {
        let y = PADDING.y + row as f32 * ROW_HEIGHT;
        let row_start = row * BYTES_PER_ROW;
        let row_end = (row_start + BYTES_PER_ROW).min(len);
        shapes.extend(push_text(
            Pos2::new(PADDING.x, y),
            &format!("{row_start:08x}"),
            COLORS.gutter_text,
        ));
        for (col, byte) in bytes[row_start..row_end].iter().enumerate() {
            let index = row_start + col;
            let hex_pos = Pos2::new(PADDING.x + geometry.hex_x[col], y);
            let ascii_pos = Pos2::new(PADDING.x + geometry.ascii_x[col], y);
            if selected.contains(&index) {
                shapes.push(PageShape::Rect {
                    rect: Rect::from_min_size(
                        hex_pos,
                        Vec2::new(geometry.char_width * 2.0, ROW_HEIGHT),
                    ),
                    corner_radius: 0.0,
                    color: COLORS.widget,
                });
                shapes.push(PageShape::Rect {
                    rect: Rect::from_min_size(
                        ascii_pos,
                        Vec2::new(geometry.char_width, ROW_HEIGHT),
                    ),
                    corner_radius: 0.0,
                    color: COLORS.widget,
                });
            }
            shapes.extend(push_text(hex_pos, &format!("{byte:02x}"), Color32::WHITE));
            let character = match byte.is_ascii_graphic() || *byte == b' ' {
                true => *byte as char,
                false => '.',
            };
            shapes.extend(push_text(ascii_pos, &character.to_string(), Color32::WHITE));
        }
    }
    if selected.is_empty()
        && let Some(focus) = focus
        && focus / BYTES_PER_ROW < rows
    {
        let row = focus / BYTES_PER_ROW;
        let col = focus % BYTES_PER_ROW;
        shapes.push(PageShape::Rect {
            rect: Rect::from_min_size(
                Pos2::new(
                    PADDING.x + geometry.hex_x[col] - 1.0,
                    PADDING.y + row as f32 * ROW_HEIGHT,
                ),
                Vec2::new(2.0, ROW_HEIGHT),
            ),
            corner_radius: 0.0,
            color: Color32::WHITE,
        });
    }
    Page::new(shapes)
}

#[component]
pub(crate) fn HexView(state: Shared) -> NodeId {
    let size = component_size();
    let canvas = NodeRef::new();
    let (scale, set_scale) = create_signal(None::<f32>);
    each_frame(move || {
        let now = with_document(|document| document.pixels_per_point());
        set_scale.set(Some(now));
    });
    let geometry = create_memo(clone!(scale -> move || {
        scale.get();
        let char_width = layout_text("0", FontId::monospace(TEXT_SIZE), TextLayout::DEFAULT)
            .map_or(TEXT_SIZE * 0.6, |galley| galley.size().x)
            .max(1.0);
        HexGeometry::new(char_width)
    }));
    let content = state.text.content();
    let rows = create_memo(clone!(state content -> move || {
        content.get();
        state.text.bytes().len().div_ceil(BYTES_PER_ROW).max(1)
    }));
    let content_size = create_memo(clone!(geometry rows size -> move || {
        let available = size.get();
        Vec2::new(
            (geometry.get().total_width + PADDING.x * 2.0).max(available.x),
            (rows.get() as f32 * ROW_HEIGHT + PADDING.y * 2.0).max(available.y),
        )
    }));
    let width = create_memo(clone!(content_size -> move || content_size.get().x));
    let height = create_memo(clone!(content_size -> move || content_size.get().y));
    let drawn = create_memo(clone!(state content geometry content_size -> move || {
        content.get();
        state.text.cursors().get();
        page(&state, &geometry.get(), content_size.get())
    }));
    let draw: Prop<Draw> = Prop::Dynamic(Rc::new(move || drawn.get().draw()));

    let cx = Rc::new(HexSurface {
        state: state.clone(),
        geometry: geometry.clone(),
        rows,
        canvas: canvas.clone(),
    });
    let press_cx = cx.clone();
    let drag_cx = cx.clone();
    let release_state = state.clone();
    let key_state = state.clone();
    let text_state = state.clone();
    view! {
        <Frame color={COLORS.surface}>
            <Scroll focus_color={Color32::TRANSPARENT}>
                <Focusable
                    on_key={move |press: KeyPress| key(&key_state, press)}
                    on_text={move |typed: String| type_text(&text_state, &typed)}
                >
                    <ClickCatcher
                        cursor=CursorIcon::Text
                        on_press={move |event: PointerPress| hex_press(&press_cx, event)}
                        on_drag={move |event: PointerPress| hex_drag(&drag_cx, event)}
                        on_active_change={move |active: bool| {
                            if !active {
                                release_state.hex_selection_anchor.set(None);
                            }
                        }}
                    >
                        <Frame @node_ref=&canvas width={width} height={height}>
                            <Drawing draw={draw} />
                        </Frame>
                    </ClickCatcher>
                </Focusable>
            </Scroll>
        </Frame>
    }
}

struct HexSurface {
    state: Shared,
    geometry: Memo<HexGeometry>,
    rows: Memo<usize>,
    canvas: NodeRef,
}

impl HexSurface {
    fn local(&self, pos: Pos2) -> Option<Vec2> {
        let node = self.canvas.try_get()?;
        let rect = with_document(|document| document.node_rect(node))?;
        Some(Vec2::new(
            pos.x - rect.min.x - PADDING.x,
            pos.y - rect.min.y - PADDING.y,
        ))
    }

    fn target(&self, pos: Pos2) -> Option<usize> {
        let local = self.local(pos)?;
        let len = document_len(&self.state);
        Some(
            self.geometry
                .get_untracked()
                .byte_at(local, self.rows.get_untracked())
                .min(len),
        )
    }
}

fn hex_press(cx: &Rc<HexSurface>, press: PointerPress) {
    let Some(target) = cx.target(press.pos) else {
        return;
    };
    cx.state.hex_pending_nibble.set(None);
    cx.state.hex_selection_anchor.set(Some(target));
    select_byte(&cx.state, target);
}

fn hex_drag(cx: &Rc<HexSurface>, press: PointerPress) {
    let Some(anchor) = cx.state.hex_selection_anchor.get() else {
        return;
    };
    let Some(target) = cx.target(press.pos) else {
        return;
    };
    set_range(&cx.state, anchor, target);
}
