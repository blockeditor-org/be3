use std::ops::Range;
use std::rc::Rc;

use beui::reactive::{
    Canvas, CanvasItem, ClickCatcher, Draw, Drawing, Focusable, ForEach, Frame, Memo, NodeRef,
    Prop, ReadSignal, Show, WriteSignal, clone, component, component_size, copy_text, create_memo,
    create_signal, each_frame, request_paste, view, with_document,
};
use beui::styled::{Button, ButtonVariant, ContextMenu, use_theme};
use beui::unstyled::{MenuItem, Scroll};
use beui::{
    Color32, CursorIcon, Key, KeyPress, NodeId, PointerPress, Pos2, Rect, ScrollPosition, Vec2,
};
use block_editor_plugin::{Drag, block_ui::BlockLabel};
use text_editor_core::{
    CursorHorizontalPositionMetric, CursorLeftRightStop, DragSelectionMode, EditorCommand,
    LRDirection, MarkdownCommand, MoveMode, SyntaxNodeDirection, UDDirection, VerticalMoveMode,
};

use crate::layout::{DocumentLayout, hit_test, layout_document};
use crate::palette;

use super::embeds::poll_pending_embeds;
use super::find::{close_find, find_step, open_find};
use super::large_embed::{LargeEmbed, embed_is_live};
use super::shapes::{
    self, PADDING, SelectionHandle, TOUCH_HANDLE_HIT_RADIUS, checkbox_at, gutter_arrow_at,
    touch_handle_anchor, touch_handle_center,
};
use super::state::{FocusedEmbed, Layout, Shared};

const REVEAL_MARGIN: Vec2 = Vec2::new(8.0, 3.0);
const EMBED_BUTTON_GAP: f32 = 6.0;
const EMBED_BUTTON_SIZE: Vec2 = Vec2::new(72.0, 30.0);

pub(crate) struct Surface {
    state: Shared,
    layout: Memo<Layout>,
    gutter: Memo<f32>,
    canvas: NodeRef,
    scroll: ReadSignal<ScrollPosition>,
    set_offset: WriteSignal<f32>,
    set_focused: WriteSignal<bool>,
    set_menu_at: WriteSignal<Option<Pos2>>,
}

type Context = Rc<Surface>;

impl Surface {
    fn origin(&self) -> Vec2 {
        shapes::origin(self.gutter.get_untracked())
    }

    fn layout(&self) -> Rc<DocumentLayout> {
        self.layout.get_untracked().0
    }

    fn local(&self, pos: Pos2) -> Option<Pos2> {
        let node = self.canvas.try_get()?;
        let rect = with_document(|document| document.node_rect(node))?;
        let origin = self.origin();
        Some(Pos2::new(
            pos.x - rect.min.x - origin.x,
            pos.y - rect.min.y - origin.y,
        ))
    }

    fn gutter_local(&self, pos: Pos2) -> Option<Pos2> {
        let node = self.canvas.try_get()?;
        let rect = with_document(|document| document.node_rect(node))?;
        Some(Pos2::new(pos.x - rect.min.x, pos.y - rect.min.y))
    }

    fn reveal_caret(&self) {
        if !self.state.reveal_cursor.replace(false) {
            return;
        }
        let position = self.scroll.get_untracked();
        if position.viewport <= 0.0 {
            return;
        }
        let layout = self.layout();
        let Some(byte) = self.state.caret_indices().first().copied() else {
            return;
        };
        let Some(rect) = shapes::caret_rect(&layout, byte, self.origin()) else {
            return;
        };
        let mut offset = position.offset;
        if rect.min.y - REVEAL_MARGIN.y < offset {
            offset = rect.min.y - REVEAL_MARGIN.y;
        }
        if rect.max.y + REVEAL_MARGIN.y > offset + position.viewport {
            offset = rect.max.y + REVEAL_MARGIN.y - position.viewport;
        }
        self.set_offset
            .set_unconditionally(offset.clamp(0.0, position.max_offset()));
    }

    fn reveal_rect(&self, rect: Rect) {
        let position = self.scroll.get_untracked();
        if position.viewport <= 0.0 {
            return;
        }
        let middle = rect.center().y - position.viewport / 2.0;
        self.set_offset
            .set_unconditionally(middle.clamp(0.0, position.max_offset()));
    }

    fn selection_handles(&self) -> Option<Range<usize>> {
        if !self.state.touch_mode.get() {
            return None;
        }
        let range = self.state.selection_ranges().into_iter().next()?;
        (range.start != range.end).then_some(range)
    }

    fn selection_handle_at(&self, local: Pos2) -> Option<SelectionHandle> {
        let range = self.selection_handles()?;
        let layout = self.layout();
        let point = Vec2::new(local.x, local.y);
        let start = touch_handle_center(
            touch_handle_anchor(&layout, range.start)?,
            SelectionHandle::Start,
        );
        let end = touch_handle_center(
            touch_handle_anchor(&layout, range.end)?,
            SelectionHandle::End,
        );
        if (point - start).length() <= TOUCH_HANDLE_HIT_RADIUS {
            Some(SelectionHandle::Start)
        } else if (point - end).length() <= TOUCH_HANDLE_HIT_RADIUS {
            Some(SelectionHandle::End)
        } else {
            None
        }
    }
}

fn begin_handle_drag(cx: &Context, handle: SelectionHandle, local: Pos2) {
    let Some(range) = cx.selection_handles() else {
        return;
    };
    let layout = cx.layout();
    let (fixed_byte, moving_byte) = match handle {
        SelectionHandle::Start => (range.end, range.start),
        SelectionHandle::End => (range.start, range.end),
    };
    let fixed = cx.state.core.borrow().position(fixed_byte);
    cx.state.dragging_handle.set(Some(fixed));
    let offset = shapes::hit_test_anchor(&layout, moving_byte)
        .map_or(Vec2::ZERO, |anchor| Vec2::new(local.x, local.y) - anchor);
    cx.state.dragging_handle_offset.set(offset);
    cx.state.selecting.set(false);
    drag_handle(cx, local);
}

fn drag_handle(cx: &Context, local: Pos2) {
    let Some(fixed) = cx.state.dragging_handle.get() else {
        return;
    };
    let layout = cx.layout();
    let offset = cx.state.dragging_handle_offset.get();
    let target = hit_test(&layout, Vec2::new(local.x, local.y) - offset);
    let position = cx.state.core.borrow().position(target);
    cx.state
        .execute(EditorCommand::DragSelectionHandle { fixed, position });
}

fn press(cx: &Context, press: PointerPress) {
    cx.set_focused.set(true);
    cx.state.touch_mode.set(press.touch);
    let (Some(local), Some(gutter_local)) = (cx.local(press.pos), cx.gutter_local(press.pos))
    else {
        return;
    };
    let layout = cx.layout();
    if press.touch
        && let Some(handle) = cx.selection_handle_at(local)
    {
        begin_handle_drag(cx, handle, local);
        return;
    }
    if let Some(embed) = layout
        .embeds
        .iter()
        .find(|embed| embed.large && embed.available && embed.rect.contains(local))
    {
        let key = FocusedEmbed {
            id: embed.id,
            source_start: embed.range.start,
        };
        if embed_is_live(&cx.state, key) {
            cx.state.set_focused_embed.set(Some(key));
            cx.state.focus_confirmed.set(false);
        }
        cx.state.selecting.set(false);
        return;
    }
    let snapshot_sections = cx.state.snapshot.borrow().sections.clone();
    if let Some(line_start) = gutter_arrow_at(
        &layout,
        &snapshot_sections,
        cx.gutter.get_untracked(),
        cx.origin(),
        gutter_local,
    ) {
        let position = cx.state.core.borrow().position(line_start);
        cx.state.execute(EditorCommand::ToggleCollapseAt(position));
        return;
    }
    if press.touch {
        return;
    }
    select_at(
        cx,
        local,
        press.clicks,
        press.modifiers.shift,
        press.modifiers.alt != press.modifiers.ctrl,
    );
}

fn tap(cx: &Context, press: PointerPress) {
    if !press.touch {
        return;
    }
    if cx.state.dragging_handle.take().is_some() {
        cx.state.dragging_handle_offset.set(Vec2::ZERO);
        return;
    }
    let Some(local) = cx.local(press.pos) else {
        return;
    };
    let layout = cx.layout();
    let target = hit_test(&layout, Vec2::new(local.x, local.y));
    if cx.state.selection_contains(target) {
        cx.set_menu_at.set(Some(press.pos));
        cx.state.selecting.set(false);
        return;
    }
    select_at(cx, local, press.clicks, false, false);
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MenuAction {
    Copy,
    Cut,
    SelectAll,
}

impl MenuAction {
    const ALL: [Self; 3] = [Self::Copy, Self::Cut, Self::SelectAll];

    fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::SelectAll => "Select All",
        }
    }
}

fn menu_action(state: &Shared, action: MenuAction) {
    match action {
        MenuAction::Copy | MenuAction::Cut => {
            let mode = match action {
                MenuAction::Cut => text_editor_core::CopyMode::Cut,
                _ => text_editor_core::CopyMode::Copy,
            };
            let text = state.copy(mode);
            if !text.is_empty() {
                copy_text(text);
            }
        }
        MenuAction::SelectAll => state.execute(EditorCommand::SelectAll),
    }
}

fn select_at(cx: &Context, local: Pos2, clicks: u32, extend: bool, syntax: bool) {
    let layout = cx.layout();
    let checkboxes = cx.state.snapshot.borrow().checkboxes.clone();
    if let Some(checkbox) = checkbox_at(&layout, &checkboxes, local) {
        let position = cx.state.core.borrow().position(checkbox.line_start);
        cx.state
            .execute(EditorCommand::Markdown(MarkdownCommand::ToggleCheckbox(
                position,
            )));
        cx.state.selecting.set(false);
        return;
    }
    if let Some(embed) = layout
        .embeds
        .iter()
        .find(|embed| !embed.large && embed.rect.contains(local))
    {
        let (anchor, focus) = {
            let core = cx.state.core.borrow();
            (
                core.position(embed.range.start),
                core.position(embed.range.end),
            )
        };
        cx.state
            .execute(EditorCommand::SetSelection { anchor, focus });
        cx.state.selecting.set(false);
        return;
    }
    let target = hit_test(&layout, Vec2::new(local.x, local.y));
    if clicks >= 4 {
        cx.state.execute(EditorCommand::SelectAll);
        cx.state.selecting.set(false);
        return;
    }
    let mode = match clicks {
        2 => DragSelectionMode::select(CursorLeftRightStop::Word),
        3 => DragSelectionMode::select(CursorLeftRightStop::Line),
        _ => DragSelectionMode::default(),
    };
    let position = cx.state.core.borrow().position(target);
    cx.state.execute(EditorCommand::Click {
        position,
        mode,
        extend,
        select_syntax_node: syntax,
    });
    cx.state.selecting.set(true);
    cx.state.reveal_cursor.set(true);
    cx.reveal_caret();
}

fn extend(cx: &Context, press: PointerPress) {
    let Some(local) = cx.local(press.pos) else {
        return;
    };
    if cx.state.dragging_handle.get().is_some() {
        drag_handle(cx, local);
        return;
    }
    if !cx.state.selecting.get() || press.touch {
        return;
    }
    let layout = cx.layout();
    let target = hit_test(&layout, Vec2::new(local.x, local.y));
    let position = cx.state.core.borrow().position(target);
    cx.state.execute(EditorCommand::Drag(position));
}

fn release(cx: &Context, active: bool) {
    if active {
        return;
    }
    cx.state.selecting.set(false);
    cx.state.dragging_handle.set(None);
    cx.state.dragging_handle_offset.set(Vec2::ZERO);
}

fn insert_text(cx: &Context, text: &str) {
    if text == "\n" || text == "\r" {
        return;
    }
    cx.state.execute(EditorCommand::InsertText(text.as_bytes()));
    cx.state.reveal_cursor.set(true);
    cx.reveal_caret();
}

fn key(cx: &Context, press: KeyPress) -> bool {
    if !press.pressed {
        return false;
    }
    let handled = key_command(cx, press);
    if handled {
        cx.state.reveal_cursor.set(true);
        cx.reveal_caret();
    }
    handled
}

fn key_command(cx: &Context, press: KeyPress) -> bool {
    let state = &cx.state;
    let modifiers = press.modifiers;
    let command = modifiers.ctrl;
    match press.key {
        Key::Escape if state.find.open.get_untracked() => {
            close_find(state);
            return true;
        }
        Key::C if command => {
            let text = state.copy(text_editor_core::CopyMode::Copy);
            if !text.is_empty() {
                copy_text(text);
            }
            return true;
        }
        Key::X if command => {
            let text = state.copy(text_editor_core::CopyMode::Cut);
            if !text.is_empty() {
                copy_text(text);
            }
            return true;
        }
        Key::V if command => {
            state.paste_requested.set(true);
            return true;
        }
        _ => {}
    }

    let direction = match press.key {
        Key::ArrowLeft | Key::Home => Some(LRDirection::Left),
        Key::ArrowRight | Key::End => Some(LRDirection::Right),
        _ => None,
    };
    if let Some(direction) = direction {
        state.execute(EditorCommand::MoveCursorLeftRight {
            mode: match modifiers.shift {
                true => MoveMode::Select,
                false => MoveMode::Move,
            },
            direction,
            stop: if matches!(press.key, Key::Home | Key::End) {
                CursorLeftRightStop::Line
            } else if modifiers.alt || modifiers.ctrl {
                CursorLeftRightStop::Word
            } else {
                CursorLeftRightStop::UnicodeGraphemeCluster
            },
        });
        return true;
    }

    if matches!(press.key, Key::ArrowUp | Key::ArrowDown) {
        let direction = match press.key {
            Key::ArrowUp => UDDirection::Up,
            _ => UDDirection::Down,
        };
        if modifiers.alt && modifiers.ctrl {
            state.execute(EditorCommand::MoveCursorUpDown {
                direction,
                mode: VerticalMoveMode::Duplicate,
                metric: CursorHorizontalPositionMetric::Byte,
                stop: CursorLeftRightStop::UnicodeGraphemeCluster,
            });
        } else if modifiers.alt {
            state.execute(EditorCommand::SelectSyntaxNode(
                match direction == UDDirection::Up {
                    true => SyntaxNodeDirection::Parent,
                    false => SyntaxNodeDirection::Child,
                },
            ));
        } else {
            state.execute(EditorCommand::MoveCursorUpDown {
                direction,
                mode: match modifiers.shift {
                    true => VerticalMoveMode::Select,
                    false => VerticalMoveMode::Move,
                },
                metric: CursorHorizontalPositionMetric::Byte,
                stop: CursorLeftRightStop::UnicodeGraphemeCluster,
            });
        }
        return true;
    }

    match press.key {
        Key::Backspace | Key::Delete => state.execute(EditorCommand::Delete {
            direction: match press.key {
                Key::Backspace => LRDirection::Left,
                _ => LRDirection::Right,
            },
            stop: if modifiers.alt || modifiers.ctrl {
                CursorLeftRightStop::Word
            } else {
                CursorLeftRightStop::UnicodeGraphemeCluster
            },
        }),
        Key::Enter if command => state.execute(EditorCommand::InsertLine(match modifiers.shift {
            true => UDDirection::Up,
            false => UDDirection::Down,
        })),
        Key::Enter => state.execute(EditorCommand::Newline),
        Key::Tab => state.execute(EditorCommand::IndentSelection(match modifiers.shift {
            true => LRDirection::Left,
            false => LRDirection::Right,
        })),
        Key::A if command => state.execute(EditorCommand::SelectAll),
        Key::B if command => {
            state.execute(EditorCommand::Markdown(MarkdownCommand::Bold));
        }
        Key::I if command => {
            state.execute(EditorCommand::Markdown(MarkdownCommand::Italic));
        }
        Key::Z if command => state.execute(match modifiers.shift {
            true => EditorCommand::Redo,
            false => EditorCommand::Undo,
        }),
        Key::Y if command => state.execute(EditorCommand::Redo),
        Key::F if command => open_find(state, false),
        Key::H if command => open_find(state, true),
        Key::G if command => {
            find_step(
                state,
                match modifiers.shift {
                    true => text_editor_core::FindDirection::Previous,
                    false => text_editor_core::FindDirection::Next,
                },
            );
        }
        Key::D if command && modifiers.shift => {
            state.execute(EditorCommand::DuplicateLine(UDDirection::Down));
        }
        Key::D if command => {
            state.execute(EditorCommand::DuplicateCursor(LRDirection::Right));
        }
        Key::BracketLeft if command && modifiers.shift => {
            state.execute(match state.cursor_line_collapsed() {
                true => EditorCommand::Uncollapse,
                false => EditorCommand::Collapse,
            });
        }
        _ => return false,
    }
    true
}

fn drop_target(cx: &Context, drag: Option<Drag>) -> Option<usize> {
    let drag = drag?;
    if drag.block_id == cx.state.block.id() {
        return None;
    }
    let local = cx.local(drag.position)?;
    let layout = cx.layout();
    let byte = hit_test(&layout, Vec2::new(local.x, local.y));
    let position = cx
        .state
        .core
        .borrow()
        .cursor_stop(byte, CursorLeftRightStop::UnicodeGraphemeCluster);
    cx.state.core.borrow().position_index(position)
}

#[component]
pub(crate) fn TextSurface(state: Shared) -> NodeId {
    let theme = use_theme();
    let size = component_size();
    let canvas = NodeRef::new();
    let (scale, set_scale) = create_signal(None::<f32>);
    let (scroll, set_scroll) = create_signal(ScrollPosition::ZERO);
    let (offset, set_offset) = create_signal(0.0_f32);
    let (focused, set_focused) = create_signal(false);
    let (menu_at, set_menu_at) = create_signal(None::<Pos2>);
    each_frame(move || {
        let scale_now = with_document(|document| document.pixels_per_point());
        set_scale.set(Some(scale_now));
    });

    let content = state.content.clone();
    let total_lines = create_memo(clone!(state content -> move || {
        content.get();
        state
            .snapshot
            .borrow()
            .bytes
            .iter()
            .filter(|byte| **byte == b'\n')
            .count()
            + 1
    }));
    let gutter = create_memo(clone!(scale total_lines -> move || {
        scale.get();
        shapes::gutter_width(total_lines.get())
    }));
    let wrap_width = create_memo(clone!(size gutter -> move || {
        (size.get().x - gutter.get() - PADDING.x * 2.0).max(1.0).round()
    }));
    let layout = create_memo(clone!(state content wrap_width scale -> move || {
        content.get();
        scale.get();
        let embeds = state.embeds.get();
        let width = wrap_width.get();
        let snapshot = state.snapshot.borrow();
        let document = layout_document(
            &snapshot.bytes,
            snapshot.highlight(),
            &embeds,
            &snapshot.checkbox_markers,
            &snapshot.hidden,
            width,
        );
        Layout(Rc::new(document.unwrap_or_default()))
    }));

    let cx: Context = Rc::new(Surface {
        state: state.clone(),
        layout: layout.clone(),
        gutter: gutter.clone(),
        canvas: canvas.clone(),
        scroll: scroll.clone(),
        set_offset,
        set_focused: set_focused.clone(),
        set_menu_at: set_menu_at.clone(),
    });

    let content_size = create_memo(clone!(layout gutter size -> move || {
        let layout = layout.get();
        let available = size.get();
        Vec2::new(
            (layout.0.size.x + gutter.get()).max(available.x),
            layout.0.size.y.max(available.y),
        )
    }));
    let content_width = create_memo(clone!(content_size -> move || content_size.get().x));
    let content_height = create_memo(clone!(content_size -> move || content_size.get().y));

    let background = create_memo(clone!(state layout gutter content_size -> move || {
        shapes::background(
            &layout.get().0,
            &state.snapshot.borrow(),
            content_size.get(),
            gutter.get(),
            shapes::origin(gutter.get()),
        )
    }));
    let selection_color = theme.accent_soft.clone();
    let selection = create_memo(clone!(state layout gutter selection_color -> move || {
        state.cursors.get();
        shapes::selection(
            &layout.get().0,
            &state.selection_ranges(),
            shapes::origin(gutter.get()),
            selection_color.get(),
        )
    }));
    let text_shapes = create_memo(clone!(state layout gutter -> move || {
        shapes::content(
            &layout.get().0,
            &state.snapshot.borrow(),
            shapes::origin(gutter.get()),
        )
    }));
    let caret_color = theme.accent.clone();
    let drag = state.editor.drag();
    let overlay_cx = cx.clone();
    let overlay = create_memo(
        clone!(state layout gutter caret_color focused drag -> move || {
            state.cursors.get();
            state.presence_revision.get();
            let layout = layout.get();
            let remote = remote_cursors(&state);
            let drop_caret = drop_target(&overlay_cx, drag.get());
            shapes::overlay(shapes::Overlay {
                layout: &layout.0,
                origin: shapes::origin(gutter.get()),
                focused: focused.get(),
                selection: &state.selection_ranges(),
                carets: &state.caret_indices(),
                caret_color: caret_color.get(),
                remote: &remote,
                touch_handles: overlay_cx.selection_handles(),
                drop_caret,
            })
        }),
    );

    let selected_embed = create_memo(clone!(state layout gutter -> move || {
        state.cursors.get();
        let layout = layout.get();
        let ranges = state.selection_ranges();
        let selection = (ranges.len() == 1).then(|| ranges[0].clone())?;
        let embed = layout
            .0
            .embeds
            .iter()
            .find(|embed| !embed.large && embed.range == selection)?;
        let rect = embed.rect.translate(shapes::origin(gutter.get()));
        Some((embed.id, embed.block_type, rect))
    }));
    let open_embed = create_memo(clone!(selected_embed -> move || selected_embed.get().is_some()));
    let open_state = state.clone();
    let open_row = move || {
        let target = selected_embed.clone();
        let state = open_state.clone();
        let rect = target
            .get_untracked()
            .map(|(_, _, rect)| rect)
            .unwrap_or(Rect::ZERO);
        view! {
            <CanvasItem
                x={rect.min.x}
                y={rect.max.y + EMBED_BUTTON_GAP}
                width={EMBED_BUTTON_SIZE.x}
                height={EMBED_BUTTON_SIZE.y}
            >
                <Button
                    label="Edit"
                    variant=ButtonVariant::Secondary
                    @test_id={"text.embed.open"}
                    on_click={move || {
                        if let Some((id, block_type, _)) = target.get_untracked() {
                            state.host().open_block(id, block_type);
                        }
                    }}
                />
            </CanvasItem>
        }
    };

    let embed_keys = create_memo(clone!(layout -> move || {
        layout
            .get()
            .0
            .embeds
            .iter()
            .enumerate()
            .filter(|(_, embed)| embed.large)
            .map(|(index, _)| index)
            .collect::<Vec<usize>>()
    }));

    let press_cx = cx.clone();
    let tap_cx = cx.clone();
    let drag_cx = cx.clone();
    let release_cx = cx.clone();
    let key_cx = cx.clone();
    let text_cx = cx.clone();
    let capture_cx = cx.clone();
    let hover_cx = cx.clone();
    let (cursor, set_cursor) = create_signal(CursorIcon::Text);
    let embed_row = clone!(state layout gutter -> move |index: usize| {
        let embed = layout.get_untracked().0.embeds[index].clone();
        let origin = shapes::origin(gutter.get_untracked());
        let rect = embed.rect.translate(origin);
        let state = state.clone();
        view! {
            <CanvasItem x={rect.min.x} y={rect.min.y} width={rect.width()} height={rect.height()}>
                <LargeEmbed state={state} embed={embed} />
            </CanvasItem>
        }
    });
    let reveal_state = state.clone();
    let reveal_cx = cx.clone();
    each_frame(move || {
        poll_pending_embeds(&reveal_state);
        poll_paste(&reveal_state);
        poll_drag(&reveal_cx);
        reveal_state.poll_external_edit();
        reveal_state.refresh_embeds();
        reveal_state.poll_presence(reveal_state.editor.presence_visible().get_untracked());
        if let Some(client_id) = reveal_state.editor.revealed().get_untracked() {
            let layout = reveal_cx.layout();
            if let Some(rect) = reveal_state.presence_cursor_rect(client_id, &layout) {
                reveal_cx.reveal_rect(rect.translate(reveal_cx.origin()));
            }
        }
        reveal_cx.reveal_caret();
    });

    view! {
        <Frame color={palette::SURFACE}>
            <Scroll
                offset={offset}
                focus_color={Color32::TRANSPARENT}
                on_change={move |position: ScrollPosition| set_scroll.set(position)}
            >
                <Focusable
                    focused={focused.clone()}
                    on_focus_change={move |is_focused: bool| set_focused.set(is_focused)}
                    on_text={move |typed: String| insert_text(&text_cx, &typed)}
                    on_key={move |press: KeyPress| key(&key_cx, press)}
                >
                    <ContextMenu
                        open_at={menu_at}
                        on_close={clone!(set_menu_at -> move || set_menu_at.set(None))}
                        items={view! {
                            <ForEach keys={MenuAction::ALL.to_vec()}>
                                {move |action: MenuAction| view! {
                                    <MenuItem label={action.label().to_owned()} />
                                }}
                            </ForEach>
                        }}
                        on_select={clone!(state -> move |path: Vec<usize>| {
                            let Some(action) = path.first().and_then(|index| {
                                MenuAction::ALL.get(*index).copied()
                            }) else {
                                return;
                            };
                            menu_action(&state, action);
                        })}
                    >
                        <ClickCatcher
                            @test_id={"text.surface"}
                            cursor={cursor}
                            capture_at={move |pos: Pos2| {
                                capture_cx
                                    .local(pos)
                                    .and_then(|local| capture_cx.selection_handle_at(local))
                                    .is_some()
                            }}
                            on_press={move |event: PointerPress| press(&press_cx, event)}
                            on_click_at={move |event: PointerPress| tap(&tap_cx, event)}
                            on_drag={move |event: PointerPress| extend(&drag_cx, event)}
                            on_active_change={move |active: bool| release(&release_cx, active)}
                            on_hover_move={move |event: PointerPress| {
                                set_cursor.set(hover_cursor(&hover_cx, event.pos));
                            }}
                        >
                            <Canvas
                                @node_ref=&canvas
                                width={content_width.clone()}
                                height={content_height.clone()}
                            >
                                <Layer page={background} size={content_size.clone()} />
                                <Layer page={selection} size={content_size.clone()} />
                                <Layer page={text_shapes} size={content_size.clone()} />
                                <ForEach keys={embed_keys} view={embed_row} />
                                <Layer page={overlay} size={content_size} />
                                <Show condition={open_embed} then={open_row} />
                            </Canvas>
                        </ClickCatcher>
                    </ContextMenu>
                </Focusable>
            </Scroll>
        </Frame>
    }
}

#[component]
fn Layer(page: Memo<shapes::Page>, size: Memo<Vec2>) -> CanvasItem {
    let width = create_memo(clone!(size -> move || size.get().x));
    let height = create_memo(clone!(size -> move || size.get().y));
    let draw: Prop<Draw> = Prop::Dynamic(Rc::new(move || page.get().draw()));
    view! {
        <CanvasItem x=0.0 y=0.0 width={width} height={height}>
            <Drawing draw={draw} />
        </CanvasItem>
    }
}

fn hover_cursor(cx: &Context, pos: Pos2) -> CursorIcon {
    let (Some(local), Some(gutter_local)) = (cx.local(pos), cx.gutter_local(pos)) else {
        return CursorIcon::Text;
    };
    let layout = cx.layout();
    let sections = cx.state.snapshot.borrow().sections.clone();
    let checkboxes = cx.state.snapshot.borrow().checkboxes.clone();
    let pointing = checkbox_at(&layout, &checkboxes, local).is_some()
        || gutter_arrow_at(
            &layout,
            &sections,
            cx.gutter.get_untracked(),
            cx.origin(),
            gutter_local,
        )
        .is_some();
    match pointing {
        true => CursorIcon::PointingHand,
        false => CursorIcon::Text,
    }
}

fn remote_cursors(state: &Shared) -> Vec<(Range<usize>, usize, Color32)> {
    let colors = state.presence_colors();
    let core = state.core.borrow();
    state
        .remote_cursors()
        .into_iter()
        .filter_map(|(client_id, cursor)| {
            let color = colors.get(&client_id).copied()?;
            let range = core.selection_range(&text_editor_core::CursorPosition::range(
                cursor.anchor,
                cursor.focus,
            ))?;
            let focus = core.position_index(cursor.focus)?;
            Some((range, focus, presence_color(color)))
        })
        .collect()
}

fn presence_color(color: block_client::presence::PresenceColor) -> Color32 {
    let rgb = block_editor_plugin::block_ui::presence_color(color);
    let [red, green, blue, alpha] = rgb.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(red, green, blue, alpha)
}

fn poll_drag(cx: &Context) {
    let Some(drag) = cx.state.editor.drag().get_untracked() else {
        return;
    };
    if drag.block_id == cx.state.block.id() {
        return;
    }
    cx.state.editor.accept_drag(true);
    if !drag.dropped {
        return;
    }
    let Some(byte) = drop_target(cx, Some(drag)) else {
        return;
    };
    let position = cx.state.core.borrow().position(byte);
    cx.state.execute(EditorCommand::SetSelection {
        anchor: position,
        focus: position,
    });
    let types = cx.state.host().block_types();
    let name = match cx.state.client.cached_block(drag.block_id) {
        Some(cached) => BlockLabel::for_cached(types.as_ref(), &cached).name,
        None => BlockLabel::new(types.as_ref(), drag.block_type, None).name,
    };
    cx.state.insert_image_embed(drag.block_id, &name);
    cx.state.reveal_cursor.set(true);
}

fn poll_paste(state: &Shared) {
    let asked = state.paste_requested.take();
    let pasted = state.paster.borrow_mut().paste(state.host(), asked);
    let Some(pasted) = pasted else {
        return;
    };
    match pasted {
        block_editor_plugin::PastedImage::Image { name, data } => {
            let image = block_client::blocks::image::Image::new(name, data);
            let source_name = image.source_name().to_owned();
            let id = state.create_image_block(image);
            state.insert_image_embed(id, &source_name);
            state.set_import_error.set(None);
        }
        block_editor_plugin::PastedImage::Failed(error) => {
            state.set_import_error.set(Some(error));
        }
        block_editor_plugin::PastedImage::Empty => {
            request_paste();
        }
    }
}
