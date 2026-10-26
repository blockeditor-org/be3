use std::cell::Cell;
use std::rc::Rc;

pub use beui_core::base::frame::{Align2, Extent, Outline, Sides};
pub use beui_core::base::{Align, Direction, ItemSize, Justify, Sizing, Track};
pub use beui_core::painter::Corners;
pub use beui_core::tree::{
    BareChild, BuildsNode, FromPlaced, ListChild, ParentKind, Placed, WithSizing,
    component_accessibility, component_placed, with_sizing,
};
pub use beui_tree::reactive::*;

use beui_core::document::Document;
use beui_core::geometry::Rect;
use beui_core::input::Key;
use beui_core::node::NodeId;

pub use beui_core::animation::{
    Animation, AnimationFrame, Easing, Fling, Motion, Spring, create_animation, motion, tween,
};
pub use beui_core::callback::NodeRef;
pub use beui_core::timer::{Timer, create_timer, now, pin_wall_clock, wall_clock};

pub use beui_core::current::enter;
pub use beui_core::current::{try_with_document, with_document, with_reactive_scope};

pub type Child = NodeId;

pub type Render<H = (), C = NodeId> = beui_tree::reactive::Render<H, C>;

pub type RenderFn<H, C = NodeId> = beui_tree::reactive::RenderFn<H, C>;

pub use beui_core::tree::with_sizing as sizing;

pub fn test_id<T: BuildsNode>(built: T, value: impl IntoProp<String>) -> T {
    bind_test_id(built.built_node(), value.into_prop());
    built
}

pub fn node_ref<T: BuildsNode>(built: T, reference: &NodeRef) -> T {
    reference.fill(built.built_node());
    built
}

pub fn build(f: impl FnOnce() -> crate::primitives::Root) -> Document {
    let mut document = Document::new();
    let root = enter(&mut document, f);
    document.set_root(root.child());
    document
}

pub fn node_rect(node: NodeId) -> ReadSignal<Rect> {
    with_document(|document| document.watch_placement(node))
}

pub fn text_bounds(node: NodeId) -> Option<ReadSignal<Rect>> {
    with_document(|document| {
        let text = document
            .arena
            .kind_of::<beui_core::base::text::TextNode>(node)?;
        Some(document.watch_text_bounds(text))
    })
}

pub fn node_placed(node: NodeId) -> ReadSignal<bool> {
    with_document(|document| document.watch_placed(node))
}

pub fn layout_text(
    text: &str,
    font: beui_core::font::FontId,
    layout: beui_core::font::TextLayout,
) -> Option<beui_core::font::Galley> {
    try_with_document(|document| document.layout_text(text, font, layout)).flatten()
}

pub fn rich_layout(
    pieces: &[beui_core::rich::Piece],
    options: beui_core::rich::RichOptions,
) -> Option<beui_core::rich::RichLayout> {
    try_with_document(|document| document.rich_layout(pieces, options)).flatten()
}

pub fn copy_text(text: impl Into<String>) {
    let text = text.into();
    with_document(|document| document.copy_text(text));
}

pub fn copy_image(png: Vec<u8>) {
    with_document(|document| document.copy_image(png));
}

pub use beui_core::clipboard::{PasteKind, Pasted};

pub fn request_paste() {
    with_document(Document::request_paste);
}

pub fn type_events(events: Vec<beui_core::input::Input>) {
    with_document(|document| {
        if let Some(context) = document.context() {
            context.type_events(events);
        }
    });
}

pub fn request_typed_paste() {
    with_document(|document| {
        if let Some(context) = document.context() {
            context.request_typed_paste();
        }
    });
}

pub fn toggle_keyboard() {
    with_document(|document| {
        if let Some(context) = document.context() {
            context.toggle_keyboard();
        }
    });
}

pub fn close_keyboard() {
    with_document(|document| {
        if let Some(context) = document.context() {
            context.close_keyboard();
        }
    });
}

pub fn pixels_per_point() -> ReadSignal<f32> {
    with_document(|document| document.watch_pixels_per_point())
}

pub fn use_screens() -> ReadSignal<Vec<beui_core::screens::Screen>> {
    with_document(|document| document.watch_screens())
}

pub fn use_fullscreen() -> ReadSignal<bool> {
    with_document(|document| document.watch_fullscreen())
}

pub fn touched_by_finger() -> bool {
    try_with_document(|document| document.touched_by_finger()).unwrap_or(false)
}

pub fn use_keyboard() -> ReadSignal<bool> {
    with_document(|document| document.watch_keyboard())
}

pub fn focus_ring(focused: impl IntoProp<bool>) -> Memo<bool> {
    let focused = focused.into_prop();
    let visible = with_document(|document| document.watch_focus_visible());
    let keyboard = with_document(|document| document.watch_keyboard());
    create_memo(move || focused.get() && visible.get() && keyboard.get())
}

pub fn holds_focus(node: NodeRef) -> Memo<bool> {
    let focused = with_document(|document| document.watch_focused());
    create_memo(move || {
        focused.get().is_some()
            && with_document(|document| {
                node.try_get()
                    .is_some_and(|node| document.focus_is_within(node))
            })
    })
}

pub fn focus_outline(
    focused: impl IntoProp<bool>,
    color: impl IntoProp<beui_core::color::Color32>,
    width: f32,
    offset: f32,
) -> Memo<Option<Outline>> {
    let ring = focus_ring(focused);
    let color = color.into_prop();
    create_memo(move || {
        ring.get()
            .then(|| Outline::new(color.get(), width).offset(offset))
    })
}

pub fn on_key(
    at: beui_core::keys::KeyAt,
    phase: beui_core::keys::KeyPhase,
    handler: impl Fn(beui_core::keys::KeyEvent) -> bool + 'static,
) {
    listen(at, phase, true, handler);
}

pub fn on_scroll_key(
    at: beui_core::keys::KeyAt,
    phase: beui_core::keys::KeyPhase,
    handler: impl Fn(beui_core::keys::KeyEvent) -> bool + 'static,
) {
    listen(at, phase, false, handler);
}

fn listen(
    at: beui_core::keys::KeyAt,
    phase: beui_core::keys::KeyPhase,
    reveal: bool,
    handler: impl Fn(beui_core::keys::KeyEvent) -> bool + 'static,
) {
    let handler: Rc<beui_core::keys::KeyHandler> = Rc::new(handler);
    with_document(|document| document.listen_keys(at, phase, reveal, Rc::downgrade(&handler)));
    on_cleanup(move || drop(handler));
}

pub fn arrow_focus(row: NodeRef, direction: Direction) {
    let (back, forth) = match direction {
        Direction::Horizontal => (Key::ArrowLeft, Key::ArrowRight),
        Direction::Vertical => (Key::ArrowUp, Key::ArrowDown),
    };
    on_key(
        beui_core::keys::KeyAt::Node(row),
        beui_core::keys::KeyPhase::Bubble,
        move |event| {
            let Some(press) = event.press() else {
                return false;
            };
            if press.modifiers.ctrl || press.modifiers.alt || press.modifiers.shift {
                return false;
            }
            with_document(|document| {
                let Some(row) = row.try_get() else {
                    return false;
                };
                let stops = document.tab_stops_within(row);
                let Some(at) = document
                    .focused_node()
                    .and_then(|focused| stops.iter().position(|stop| *stop == focused))
                else {
                    return false;
                };
                let to = match press.key {
                    key if key == back => at.checked_sub(1),
                    key if key == forth => Some(at + 1).filter(|to| *to < stops.len()),
                    Key::Home => Some(0),
                    Key::End => Some(stops.len() - 1),
                    _ => None,
                };
                let Some(to) = to else {
                    return false;
                };
                if press.pressed {
                    document.focus_focusable(stops[to]);
                }
                true
            })
        },
    );
}

pub fn on_shortcut(shortcut: impl Fn(beui_core::input::KeyPress) -> bool + 'static) {
    let at = match beui_tree::reactive::try_current_component() {
        Some(_) => beui_core::keys::KeyAt::Node(beui_core::tree::component_node()),
        None => beui_core::keys::KeyAt::Root,
    };
    on_key(
        at,
        beui_core::keys::KeyPhase::Capture,
        move |event| match (event.press(), event.phase) {
            (Some(press), beui_core::keys::KeyPhase::Capture) => shortcut(press),
            _ => false,
        },
    );
}

pub fn on_global_key(handler: impl Fn(beui_core::keys::KeyEvent) -> bool + 'static) {
    on_key(
        beui_core::keys::KeyAt::Window,
        beui_core::keys::KeyPhase::Capture,
        handler,
    );
}

pub fn held_modifiers() -> ReadSignal<beui_core::input::Modifiers> {
    with_document(|document| document.watch_modifiers())
}

pub fn on_finger_tap(tap: impl Fn(usize) -> bool + 'static) {
    let tap: Rc<beui_core::document::FingerTap> = Rc::new(tap);
    with_document(|document| document.register_finger_tap(Rc::downgrade(&tap)));
    on_cleanup(move || drop(tap));
}

pub fn last_pointer() -> Option<beui_core::input::PointerSample> {
    try_with_document(|document| document.last_pointer).flatten()
}

pub fn focus_takes_text() -> bool {
    with_document(|document| document.focus_takes_text())
}

pub fn bind(node: NodeId, effect: impl FnMut() + 'static) {
    let owner = with_document(|document| document.arena.owner(node));
    match owner.filter(ScopeContext::is_alive) {
        Some(owner) => owner.run(|| create_effect(effect)),
        None => create_effect(effect),
    };
}

pub fn bind_test_id(node: NodeId, test_id: Prop<String>) {
    let reading = match test_id {
        Prop::Static(value) => {
            with_document(|document| document.set_test_id(node, value));
            return;
        }
        Prop::Dynamic(reading) => reading,
    };
    let published: Cell<Option<String>> = Cell::new(None);
    bind(node, move || {
        let next = reading.get();
        let previous = published.replace(Some(next.clone()));
        with_document(|document| {
            if let Some(previous) = previous
                && previous != next
            {
                document.clear_test_id(node, &previous);
            }
            document.set_test_id(node, next);
        });
    });
}

pub use crate::actions::{
    Action, ActionBuilder, ActionScope, Chord, MenuNode, Pin, Submenu, action_checked,
    action_disabled, action_glyph, action_label, action_scope, action_tooltip, active_actions,
    active_actions_from, add_to_menu, document_menu, menu_action, tidy_menu,
};
pub use crate::component_ref::ComponentRef;
pub use crate::file_picker::{
    FileFilter, FilePick, FilePicker, PickedFile, create_file_picker, pick_file,
};
pub use crate::file_saver::{FileSave, SavedFile, save_file};
pub use crate::nodes::*;
pub use beui_core::base::canvas::CanvasView;
pub use beui_core::base::drawing::{Draw, draw_gpu};
pub use beui_core::base::embed::{EmbedPlacement, EmbedSlot};
pub use beui_core::base::overlay::{OverlayAnchor, Placement};
pub use beui_core::layer::{Dismiss, LayerLevel, LayerPolicy};
pub use beui_core::rich::{
    CaretHandle, Piece, RichLayout, SpanBackground, SpanKind, SpanStyle, TextCaret, TextGlyph,
    TextMark,
};
