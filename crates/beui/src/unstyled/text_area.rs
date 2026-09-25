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
use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{CursorIcon, Key, KeyPress, PointerPress};
use crate::node::NodeId;
use crate::page::Page;
use crate::reactive::{
    Callback, Canvas, CanvasItem, Children, ClickCatcher, Draw, Drawing, Focusable, Frame, Memo,
    Prop, ReadSignal, WriteSignal, clone, component_accessibility, component_size, copy_text,
    create_effect, create_memo, create_signal, request_paste, untrack, use_pixels_per_point,
};
use crate::unstyled::Scroll;

use layout::{hit_test, layout_document};
use shapes::{
    PADDING, SelectionHandle, TOUCH_HANDLE_HIT_RADIUS, checkbox_at, gutter_arrow_at,
    touch_handle_anchor, touch_handle_center,
};

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
    set_focused: WriteSignal<bool>,
    on_widget_press: Callback<usize, bool>,
    on_menu: Callback<Pos2>,
}

type Context = Rc<Surface>;

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
        if !self.state.touch_mode() {
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
            touch_handle_anchor(layout.document(), range.start)?,
            SelectionHandle::Start,
        );
        let end = touch_handle_center(
            touch_handle_anchor(layout.document(), range.end)?,
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
    let fixed = cx.state.core().position(fixed_byte);
    cx.state.begin_handle_drag(fixed);
    let offset = shapes::hit_test_anchor(layout.document(), moving_byte)
        .map_or(Vec2::ZERO, |anchor| Vec2::new(local.x, local.y) - anchor);
    cx.state.set_handle_offset(offset);
    drag_handle(cx, local);
}

fn drag_handle(cx: &Context, local: Pos2) {
    let Some(fixed) = cx.state.dragging_handle() else {
        return;
    };
    let layout = cx.layout();
    let offset = cx.state.handle_offset();
    let target = hit_test(layout.document(), Vec2::new(local.x, local.y) - offset);
    let position = cx.state.core().position(target);
    cx.state
        .execute(EditorCommand::DragSelectionHandle { fixed, position });
}

fn press(cx: &Context, press: PointerPress) {
    cx.set_focused.set(true);
    cx.state.set_touch_mode(press.touch);
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
    if cx.state.end_handle_drag() {
        return;
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
    select_at(cx, local, press.clicks, false, false);
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
    let Some(local) = cx.local(press.pos) else {
        return;
    };
    if cx.state.dragging_handle().is_some() {
        drag_handle(cx, local);
        return;
    }
    if !cx.state.selecting() || press.touch {
        return;
    }
    let layout = cx.layout();
    let target = hit_test(layout.document(), Vec2::new(local.x, local.y));
    let position = cx.state.core().position(target);
    cx.state.execute(EditorCommand::Drag(position));
}

fn release(cx: &Context, active: bool) {
    if active {
        return;
    }
    cx.state.set_selecting(false);
    cx.state.end_handle_drag();
}

fn insert_text(cx: &Context, text: &str) {
    if text == "\n" || text == "\r" {
        return;
    }
    cx.state.execute(EditorCommand::InsertText(text.as_bytes()));
    cx.state.reveal_cursor();
}

fn key(cx: &Context, press: KeyPress) -> bool {
    if !press.pressed {
        return false;
    }
    let handled = key_command(cx, press);
    if handled {
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
    on_widget_press: Callback<usize, bool>,
    on_menu: Callback<Pos2>,
    on_key_override: Callback<KeyPress, bool>,
    children: Children<CanvasItem>,
) -> NodeId {
    let size = component_size();
    let canvas = state.canvas();
    let scale = use_pixels_per_point();
    let (scroll, set_scroll) = create_signal(ScrollPosition::ZERO);
    let (offset, set_offset) = create_signal(0.0_f32);
    let (focused, set_focused) = create_signal(false);

    let accessible = state.content();
    component_accessibility(create_memo(clone!(state accessible -> move || {
        accessible.get();
        let mut node = Node::new(Role::MultilineTextInput);
        node.set_value(String::from_utf8_lossy(&state.bytes()).into_owned());
        node
    })));

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
        clone!(state content wrap_width scale gutter widgets -> move || {
            content.get();
            scale.get();
            let widgets = widgets.get();
            let width = wrap_width.get();
            let document = state.with_snapshot(|snapshot| {
                layout_document(
                    &snapshot.bytes,
                    snapshot.highlight(),
                    &widgets,
                    &snapshot.checkbox_markers,
                    &snapshot.hidden,
                    width,
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
        set_focused: set_focused.clone(),
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
    let text_shapes = create_memo(clone!(state layout colors -> move || {
        let layout = layout.get();
        state.with_snapshot(|snapshot| {
            shapes::content(layout.document(), snapshot, &colors.get(), layout.origin())
        })
    }));
    let overlay_cx = cx.clone();
    let overlay = create_memo(
        clone!(state layout colors focused remote_cursors drop_caret -> move || {
            state.cursors().get();
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
    let reveal_cx = cx.clone();
    let reveals = state.reveals();
    create_effect(move || {
        reveals.get();
        untrack(|| reveal_cx.reveal_caret());
    });
    let (cursor, set_cursor) = create_signal(CursorIcon::Text);
    let surface_color = create_memo(clone!(colors -> move || colors.get().surface));

    view! {
        <Frame color={surface_color}>
            <Scroll
                offset={offset}
                focus_color={Color32::TRANSPARENT}
                on_change={move |position: ScrollPosition| set_scroll.set(position)}
            >
                <Focusable
                    focused={focused.clone()}
                    on_focus_change={move |is_focused: bool| set_focused.set(is_focused)}
                    on_text={move |typed: String| insert_text(&text_cx, &typed)}
                    on_key={move |press: KeyPress| {
                        on_key_override.call(press) || key(&key_cx, press)
                    }}
                >
                    <ClickCatcher
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
                            {children}
                            <Layer page={overlay} size={content_size} />
                        </Canvas>
                    </ClickCatcher>
                </Focusable>
            </Scroll>
        </Frame>
    }
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
