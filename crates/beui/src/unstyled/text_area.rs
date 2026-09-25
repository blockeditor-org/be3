mod colors;
mod layout;
mod shapes;
mod state;

use std::ops::Range;
use std::rc::Rc;

use accesskit::{Node, Role};

use text_editor_core::{
    CopyMode, CursorHorizontalPositionMetric, CursorLeftRightStop, DragSelectionMode,
    EditorCommand, FindDirection, LRDirection, MarkdownCommand, MoveMode, SyntaxNodeDirection,
    UDDirection, VerticalMoveMode,
};

use beui_macros::{component, view};

use crate::base::ScrollPosition;
use crate::color::Color32;
use crate::document::Document;
use crate::font::FontId;
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{CursorIcon, Key, KeyPress, PointerPress};
use crate::node::NodeId;
use crate::page::Page;
use crate::reactive::{
    Callback, Canvas, CanvasItem, Children, ClickCatcher, Draw, Drawing, Focusable, Frame, Memo,
    NodeRef, Prop, ReadSignal, WriteSignal, clone, component_accessibility, component_size,
    copy_text, create_effect, create_memo, create_signal, request_paste, set_component_state,
    untrack,
    use_pixels_per_point, with_document,
};
use crate::unstyled::Scroll;

use layout::{BODY_SIZE, LayoutOptions, hit_test, layout_document};
use shapes::{
    PADDING, SelectionHandle, TOUCH_HANDLE_HIT_RADIUS, checkbox_at, gutter_arrow_at,
    touch_handle_anchor, touch_handle_center,
};
use state::Grab;

pub use colors::{SyntaxColors, TextAreaColors};
pub use layout::TextWidget;
pub use state::{TextAreaLayout, TextAreaState};

const REVEAL_MARGIN: Vec2 = Vec2::new(8.0, 3.0);
const SELECT_ALL_CLICKS: u32 = 4;
const WORD_CLICKS: u32 = 2;
const LINE_CLICKS: u32 = 3;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteTextCursor {
    pub selection: Range<usize>,
    pub caret: usize,
    pub color: Color32,
}

struct Surface {
    state: TextAreaState,
    layout: Memo<TextAreaLayout>,
    gutter: Memo<f32>,
    scroll: ReadSignal<ScrollPosition>,
    set_offset: WriteSignal<f32>,
    focused: ReadSignal<bool>,
    set_focused: WriteSignal<bool>,
    set_autoscroll: WriteSignal<bool>,
    viewport: NodeRef,
    masked: Memo<bool>,
    on_widget_press: Callback<usize, bool>,
    on_menu: Callback<Pos2>,
}

type Context = Rc<Surface>;

struct Parts {
    cx: Context,
    shown: Memo<String>,
}

impl Surface {
    fn origin(&self) -> Vec2 {
        shapes::origin(self.gutter.get_untracked())
    }

    fn layout(&self) -> TextAreaLayout {
        self.layout.get_untracked()
    }

    fn local(&self, pos: Pos2) -> Option<Pos2> {
        self.state.local(pos)
    }

    fn gutter_local(&self, pos: Pos2) -> Option<Pos2> {
        self.state.gutter_local(pos)
    }

    fn reveal_caret(&self) {
        let position = self.scroll.get_untracked();
        if position.viewport <= 0.0 {
            return;
        }
        if let Some(rect) = self.state.take_reveal() {
            self.reveal_rect(rect);
        }
        if !self.state.take_reveal_cursor() {
            return;
        }
        let layout = self.layout();
        let Some(byte) = self.state.caret_indices().first().copied() else {
            return;
        };
        let Some(rect) = shapes::caret_rect(layout.document(), byte, self.origin()) else {
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
        let middle = rect.center().y - position.viewport / 2.0;
        self.set_offset
            .set_unconditionally(middle.clamp(0.0, position.max_offset()));
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
            || state.selection_ranges().iter().any(|range| !range.is_empty())
        {
            return None;
        }
        state.caret_indices().first().copied()
    }

    fn handle_at(&self, local: Pos2) -> Option<SelectionHandle> {
        let layout = self.layout();
        let point = Vec2::new(local.x, local.y);
        let hit = |byte: usize, handle: SelectionHandle| {
            touch_handle_anchor(layout.document(), byte)
                .map(|anchor| touch_handle_center(anchor, handle))
                .is_some_and(|center| (point - center).length() <= TOUCH_HANDLE_HIT_RADIUS)
        };
        if let Some(caret) = self.caret_handle() {
            return hit(caret, SelectionHandle::Caret).then_some(SelectionHandle::Caret);
        }
        let range = self.selection_handles()?;
        if hit(range.start, SelectionHandle::Start) {
            Some(SelectionHandle::Start)
        } else if hit(range.end, SelectionHandle::End) {
            Some(SelectionHandle::End)
        } else {
            None
        }
    }

    fn inside(&self, pos: Pos2) -> (Pos2, Option<Beyond>) {
        let Some(rect) = self
            .viewport
            .try_get()
            .and_then(|node| with_document(|document| document.node_rect(node)))
        else {
            return (pos, None);
        };
        let bottom = (rect.max.y - 1.0).max(rect.min.y);
        let beyond = if pos.y < rect.min.y {
            Some(Beyond::Before)
        } else if pos.y > bottom {
            Some(Beyond::After)
        } else {
            None
        };
        (Pos2::new(pos.x, pos.y.clamp(rect.min.y, bottom)), beyond)
    }

    fn step_beyond(&self, beyond: Option<Beyond>, mode: MoveMode) {
        self.set_autoscroll.set(beyond.is_some());
        let Some(beyond) = beyond else {
            return;
        };
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

fn begin_handle_drag(cx: &Context, handle: SelectionHandle, local: Pos2) {
    let layout = cx.layout();
    let (grab, moving_byte) = match handle {
        SelectionHandle::Caret => {
            let Some(caret) = cx.caret_handle() else {
                return;
            };
            (Grab::Caret, caret)
        }
        SelectionHandle::Start | SelectionHandle::End => {
            let Some(range) = cx.selection_handles() else {
                return;
            };
            let (fixed, moving) = match handle {
                SelectionHandle::Start => (range.end, range.start),
                _ => (range.start, range.end),
            };
            (Grab::Selection(cx.state.core().position(fixed)), moving)
        }
    };
    let offset = shapes::hit_test_anchor(layout.document(), moving_byte)
        .map_or(Vec2::ZERO, |anchor| Vec2::new(local.x, local.y) - anchor);
    cx.state.begin_grab(grab, offset);
}

fn drag_handle(cx: &Context, pos: Pos2) {
    let Some(grab) = cx.state.grab() else {
        return;
    };
    let offset = cx.state.grab_offset();
    let (inside, beyond) = cx.inside(pos - offset);
    let Some(local) = cx.local(inside) else {
        return;
    };
    let layout = cx.layout();
    let target = hit_test(layout.document(), Vec2::new(local.x, local.y));
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
    cx.set_focused.set(true);
    cx.state.set_touch_mode(press.touch);
    cx.state.end_grab();
    let (Some(local), Some(gutter_local)) = (cx.local(press.pos), cx.gutter_local(press.pos))
    else {
        return;
    };
    let layout = cx.layout();
    if press.touch
        && let Some(handle) = cx.handle_at(local)
    {
        begin_handle_drag(cx, handle, local);
        return;
    }
    if !press.touch {
        cx.state.set_caret_handle(false);
    }
    if let Some(widget) = layout
        .document()
        .widgets
        .iter()
        .find(|widget| widget.block && widget.rect.contains(local))
        && cx.on_widget_press.call(widget.index)
    {
        cx.state.set_selecting(false);
        return;
    }
    if let Some(line_start) = gutter_arrow_at(
        layout.document(),
        &cx.state.sections(),
        cx.gutter.get_untracked(),
        cx.origin(),
        gutter_local,
    ) {
        let position = cx.state.core().position(line_start);
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
    match cx.state.end_grab() {
        Some(Grab::Caret) => {
            cx.on_menu.call(press.pos);
            return;
        }
        Some(Grab::Selection(_)) => return,
        None => {}
    }
    let Some(local) = cx.local(press.pos) else {
        return;
    };
    let layout = cx.layout();
    let target = hit_test(layout.document(), Vec2::new(local.x, local.y));
    if cx.state.selection_contains(target) {
        cx.on_menu.call(press.pos);
        cx.state.set_selecting(false);
        return;
    }
    cx.state.set_caret_handle(true);
    select_at(cx, local, press.clicks, false, false);
    cx.state.set_selecting(false);
}

fn select_at(cx: &Context, local: Pos2, clicks: u32, extend: bool, syntax: bool) {
    let layout = cx.layout();
    let checkboxes = cx.state.checkboxes();
    if let Some(checkbox) = checkbox_at(layout.document(), &checkboxes, local) {
        let position = cx.state.core().position(checkbox.line_start);
        cx.state
            .execute(EditorCommand::Markdown(MarkdownCommand::ToggleCheckbox(
                position,
            )));
        cx.state.set_selecting(false);
        return;
    }
    if let Some(widget) = layout
        .document()
        .widgets
        .iter()
        .find(|widget| !widget.block && widget.rect.contains(local))
    {
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
    let target = hit_test(layout.document(), Vec2::new(local.x, local.y));
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
    if cx.state.grab().is_some() {
        drag_handle(cx, press.pos);
        return;
    }
    if !cx.state.selecting() || press.touch {
        return;
    }
    let (inside, beyond) = cx.inside(press.pos);
    let Some(local) = cx.local(inside) else {
        return;
    };
    let layout = cx.layout();
    let target = hit_test(layout.document(), Vec2::new(local.x, local.y));
    let position = cx.state.core().position(target);
    cx.state.execute(EditorCommand::Drag(position));
    cx.step_beyond(beyond, MoveMode::Select);
    cx.state.reveal_cursor();
}

fn blur(cx: &Context) {
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
    if text == "\n" || text == "\r" {
        return;
    }
    cx.state.set_caret_handle(false);
    cx.state.execute(EditorCommand::InsertText(text.as_bytes()));
    cx.state.reveal_cursor();
}

fn key(cx: &Context, press: KeyPress) -> bool {
    if !press.pressed {
        return false;
    }
    let handled = key_command(cx, press);
    if handled {
        cx.state.set_caret_handle(false);
        cx.state.reveal_cursor();
    }
    handled
}

fn copy(state: &TextAreaState, mode: CopyMode) {
    let text = state.copy(mode);
    if !text.is_empty() {
        copy_text(text);
    }
}

fn key_command(cx: &Context, press: KeyPress) -> bool {
    let state = &cx.state;
    let modifiers = press.modifiers;
    let command = modifiers.ctrl;
    match press.key {
        Key::Escape if state.find_open().get_untracked() => {
            state.close_find();
            return true;
        }
        Key::C | Key::X if command && cx.masked.get_untracked() => return true,
        Key::C if command => {
            copy(state, CopyMode::Copy);
            return true;
        }
        Key::X if command => {
            copy(state, CopyMode::Cut);
            return true;
        }
        Key::V if command => {
            request_paste();
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
        Key::F if command => state.open_find(false),
        Key::H if command => state.open_find(true),
        Key::G if command => {
            state.find_step(match modifiers.shift {
                true => FindDirection::Previous,
                false => FindDirection::Next,
            });
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

#[component]
pub fn TextArea(
    state: TextAreaState,
    #[prop(default = Vec::new())] widgets: Prop<Vec<TextWidget>>,
    #[prop(default = TextAreaColors::DEFAULT)] colors: Prop<TextAreaColors>,
    #[prop(default = Vec::new())] remote_cursors: Prop<Vec<RemoteTextCursor>>,
    #[prop(default = None)] drop_caret: Prop<Option<usize>>,
    #[prop(default = String::new())] placeholder: Prop<String>,
    #[prop(default = false)] password: Prop<bool>,
    #[prop(default = false)] focused: Prop<bool>,
    on_widget_press: Callback<usize, bool>,
    on_menu: Callback<Pos2>,
    on_key_override: Callback<KeyPress, bool>,
    children: Children<CanvasItem>,
) -> NodeId {
    let focus_request = focused;
    let size = component_size();
    let canvas = state.canvas();
    let scale = use_pixels_per_point();
    let (scroll, set_scroll) = create_signal(ScrollPosition::ZERO);
    let (offset, set_offset) = create_signal(0.0_f32);
    let (focused, set_focused) = create_signal(false);
    let (autoscroll, set_autoscroll) = create_signal(false);
    let viewport = NodeRef::new();
    let masked = create_memo(move || password.get());
    let placeholder = create_memo(move || placeholder.get());

    let accessible = state.content();
    component_accessibility(create_memo(
        clone!(state accessible masked placeholder -> move || {
            accessible.get();
            let mut node = Node::new(Role::MultilineTextInput);
            let value = String::from_utf8_lossy(&state.bytes()).into_owned();
            node.set_value(match masked.get() {
                true => layout::mask(&value),
                false => value,
            });
            let placeholder = placeholder.get();
            if !placeholder.is_empty() {
                node.set_placeholder(placeholder);
            }
            node
        }),
    ));

    let content = state.content();
    let total_lines = create_memo(clone!(state content -> move || {
        content.get();
        state.bytes().iter().filter(|byte| **byte == b'\n').count() + 1
    }));
    let gutter = create_memo(clone!(scale total_lines -> move || {
        scale.get();
        shapes::gutter_width(total_lines.get())
    }));
    let wrap_width = create_memo(clone!(size gutter -> move || {
        (size.get().x - gutter.get() - PADDING.x * 2.0).max(1.0).round()
    }));
    let layout = create_memo(
        clone!(state content wrap_width scale gutter widgets masked -> move || {
            content.get();
            scale.get();
            let widgets = widgets.get();
            let options = LayoutOptions {
                mask: masked.get(),
                ..LayoutOptions::wrapped(wrap_width.get())
            };
            let document = state.with_snapshot(|snapshot| {
                layout_document(
                    &snapshot.bytes,
                    snapshot.highlight(),
                    &widgets,
                    &snapshot.checkbox_markers,
                    &snapshot.hidden,
                    &options,
                )
            });
            TextAreaLayout::new(
                Rc::new(document.unwrap_or_default()),
                shapes::origin(gutter.get()),
            )
        }),
    );
    create_effect(clone!(state layout -> move || state.publish_layout(layout.get())));

    let cx: Context = Rc::new(Surface {
        state: state.clone(),
        layout: layout.clone(),
        gutter: gutter.clone(),
        scroll: scroll.clone(),
        set_offset,
        focused: focused.clone(),
        set_focused: set_focused.clone(),
        set_autoscroll,
        viewport: viewport.clone(),
        masked: masked.clone(),
        on_widget_press,
        on_menu,
    });

    let content_size = create_memo(clone!(layout gutter size -> move || {
        let layout = layout.get();
        let available = size.get();
        Vec2::new(
            (layout.size().x + gutter.get()).max(available.x),
            layout.size().y.max(available.y),
        )
    }));
    let content_width = create_memo(clone!(content_size -> move || content_size.get().x));
    let content_height = create_memo(clone!(content_size -> move || content_size.get().y));

    let background = create_memo(clone!(state layout gutter colors content_size -> move || {
        let layout = layout.get();
        state.with_snapshot(|snapshot| {
            shapes::background(
                layout.document(),
                snapshot,
                &colors.get(),
                content_size.get(),
                gutter.get(),
                layout.origin(),
            )
        })
    }));
    let selection = create_memo(clone!(state layout colors -> move || {
        state.cursors().get();
        let layout = layout.get();
        shapes::selection(
            layout.document(),
            &state.selection_ranges(),
            &colors.get(),
            layout.origin(),
        )
    }));
    let text_shapes = create_memo(clone!(state layout colors placeholder -> move || {
        let layout = layout.get();
        let colors = colors.get();
        let placeholder = placeholder.get();
        let placeholder = match state.bytes().is_empty() && !placeholder.is_empty() {
            true => shapes::placeholder(
                layout.document(),
                &placeholder,
                FontId::proportional(BODY_SIZE),
                &colors,
                layout.origin(),
            ),
            false => None,
        };
        state.with_snapshot(|snapshot| {
            shapes::content(layout.document(), snapshot, &colors, layout.origin(), placeholder)
        })
    }));
    let overlay_cx = cx.clone();
    let overlay = create_memo(
        clone!(state layout colors focused remote_cursors drop_caret -> move || {
            state.cursors().get();
            state.touch_mode().get();
            state.caret_handle().get();
            let layout = layout.get();
            shapes::overlay(shapes::Overlay {
                layout: layout.document(),
                colors: &colors.get(),
                origin: layout.origin(),
                focused: focused.get(),
                selection: &state.selection_ranges(),
                carets: &state.caret_indices(),
                remote: &remote_cursors.get(),
                touch_handles: overlay_cx.selection_handles(),
                caret_handle: overlay_cx.caret_handle(),
                drop_caret: drop_caret.get(),
            })
        }),
    );

    let press_cx = cx.clone();
    let tap_cx = cx.clone();
    let drag_cx = cx.clone();
    let release_cx = cx.clone();
    let key_cx = cx.clone();
    let text_cx = cx.clone();
    let capture_cx = cx.clone();
    let hover_cx = cx.clone();
    let blur_cx = cx.clone();
    let focus_requests = state.focus_requests();
    let focus_writer = set_focused.clone();
    create_effect(move || {
        if focus_requests.get() > 0 {
            focus_writer.set(true);
        }
    });
    create_effect(clone!(set_focused -> move || {
        if focus_request.get() {
            set_focused.set(true);
        }
    }));
    let content = state.content();
    let shown = create_memo(clone!(state masked placeholder -> move || {
        content.get();
        let value = String::from_utf8_lossy(&state.bytes()).into_owned();
        match (value.is_empty(), masked.get()) {
            (true, _) => placeholder.get(),
            (false, true) => layout::mask(&value),
            (false, false) => value,
        }
    }));
    set_component_state(Parts {
        cx: cx.clone(),
        shown,
    });
    let reveal_cx = cx.clone();
    let reveals = state.reveals();
    create_effect(move || {
        reveals.get();
        untrack(|| reveal_cx.reveal_caret());
    });
    let (cursor, set_cursor) = create_signal(CursorIcon::Text);
    let surface_color = create_memo(clone!(colors -> move || colors.get().surface));

    view! {
        <Frame @node_ref=&viewport color={surface_color}>
            <Scroll
                offset={offset}
                focus_color={Color32::TRANSPARENT}
                on_change={move |position: ScrollPosition| set_scroll.set(position)}
            >
                <Focusable
                    focused={focused.clone()}
                    on_focus_change={move |is_focused: bool| {
                        set_focused.set(is_focused);
                        if !is_focused {
                            blur(&blur_cx);
                        }
                    }}
                    on_text={move |typed: String| insert_text(&text_cx, &typed)}
                    on_key={move |press: KeyPress| {
                        on_key_override.call(press) || key(&key_cx, press)
                    }}
                >
                    <ClickCatcher
                        cursor={cursor}
                        repeat_drag={autoscroll}
                        capture_at={move |pos: Pos2| {
                            capture_cx
                                .local(pos)
                                .and_then(|local| capture_cx.handle_at(local))
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
                            {children}
                            <Layer page={overlay} size={content_size} />
                        </Canvas>
                    </ClickCatcher>
                </Focusable>
            </Scroll>
        </Frame>
    }
}

pub fn text_area_shown(document: &Document, area: NodeId) -> String {
    document
        .component_state::<Parts>(area)
        .shown
        .get_untracked()
}

pub fn text_area_state(document: &Document, area: NodeId) -> TextAreaState {
    document.component_state::<Parts>(area).cx.state.clone()
}

#[cfg(test)]
pub(crate) fn text_area_handles(document: &Document, area: NodeId) -> Vec<Pos2> {
    let cx = &document.component_state::<Parts>(area).cx;
    let Some(canvas) = cx
        .state
        .canvas()
        .try_get()
        .and_then(|canvas| document.node_rect(canvas))
    else {
        return Vec::new();
    };
    let layout = cx.layout.get_untracked();
    let origin = canvas.min.to_vec2() + layout.origin();
    let center = |byte: usize, handle: SelectionHandle| {
        touch_handle_anchor(layout.document(), byte)
            .map(|anchor| {
                let center = touch_handle_center(anchor, handle) + origin;
                Pos2::new(center.x, center.y)
            })
    };
    if let Some(caret) = cx.caret_handle() {
        return center(caret, SelectionHandle::Caret).into_iter().collect();
    }
    cx.selection_handles()
        .map(|range| {
            [
                center(range.start, SelectionHandle::Start),
                center(range.end, SelectionHandle::End),
            ]
            .into_iter()
            .flatten()
            .collect()
        })
        .unwrap_or_default()
}

#[component]
fn Layer(page: Memo<Page>, size: Memo<Vec2>) -> CanvasItem {
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
    let checkboxes = cx.state.checkboxes();
    let pointing = checkbox_at(layout.document(), &checkboxes, local).is_some()
        || gutter_arrow_at(
            layout.document(),
            &cx.state.sections(),
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
