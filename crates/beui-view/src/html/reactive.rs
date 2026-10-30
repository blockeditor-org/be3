use std::cell::Cell;
use std::rc::Rc;

use beui_tree::reactive::*;

use crate::types::{KeyAt, KeyEvent, KeyPhase, NodeId, NodeRef};
use beui_core_html::keys::KeyHandler;

pub(crate) use beui_core_html::focus::{focus_ancestry, focused_node, set_focus};
pub(crate) use beui_core_html::frame::{Timer, create_timer, now, pin_wall_clock, wall_clock};
pub(crate) use beui_core_html::keys::held_modifiers;
pub(crate) use beui_core_html::page::{
    pixels_per_point, use_font_revision, use_preference_layers, use_preferences, use_viewport_size,
};

pub(crate) fn use_preference_layer(
    source: crate::types::PreferenceSource,
) -> Binding<crate::types::PreferenceLayer> {
    let layers = use_preference_layers();
    Binding::new(
        create_memo(move || layers.get().layer(source)),
        Callback::new(move |layer: crate::types::PreferenceLayer| {
            beui_core_html::page::set_preference_layer(source, layer);
        }),
    )
}

pub(crate) fn copy_text(text: String) {
    beui_core_html::page::copy_text(text);
}

pub(crate) fn copy_image(png: Vec<u8>) {
    let _ = png;
}

pub(crate) fn focus_takes_text() -> bool {
    false
}

pub(crate) fn on_finger_tap(tap: impl Fn(usize) -> bool + 'static) {
    let _ = tap;
}

pub(crate) fn use_fullscreen() -> ReadSignal<bool> {
    create_signal(false).read_only()
}

pub(crate) fn opener_of(node: NodeId) -> NodeId {
    node
}

pub(crate) fn window_of(_node: NodeId) -> crate::types::WindowId {
    crate::types::WindowId::MAIN
}

pub(crate) fn stacked_at(
    nodes: &[NodeId],
    window: crate::types::WindowId,
    pos: crate::types::Pos2,
) -> Vec<NodeId> {
    if window != crate::types::WindowId::MAIN {
        return Vec::new();
    }
    beui_core_html::nodes::stacked_at(nodes, pos)
}

pub(crate) fn focus_ring(focused: Prop<bool>) -> Memo<bool> {
    beui_core_html::focus::focus_ring(create_memo(move || focused.get()))
}

pub(crate) fn holds_focus(node: NodeRef) -> Memo<bool> {
    beui_core_html::focus::holds_focus(node)
}

pub(crate) fn on_key(at: KeyAt, phase: KeyPhase, handler: impl Fn(KeyEvent) -> bool + 'static) {
    let handler: Rc<KeyHandler> = Rc::new(handler);
    beui_core_html::keys::listen(at, phase, handler);
}

pub(crate) fn bind(node: NodeId, effect: impl FnMut() + 'static) {
    match beui_core_html::document::owner(node).filter(ScopeContext::is_alive) {
        Some(owner) => owner.run(|| create_effect(effect)),
        None => create_effect(effect),
    };
}

pub(crate) fn bind_test_id(node: NodeId, test_id: Prop<String>) {
    let reading = match test_id {
        Prop::Static(value) => {
            beui_core_html::document::add_test_id(node, value);
            return;
        }
        Prop::Dynamic(reading) => reading,
    };
    let published: Cell<Option<String>> = Cell::new(None);
    bind(node, move || {
        let next = reading.get();
        if let Some(previous) = published.replace(Some(next.clone()))
            && previous != next
        {
            beui_core_html::document::remove_test_id(node, &previous);
        }
        beui_core_html::document::add_test_id(node, next);
    });
}

pub(crate) fn tree_local<T: Clone + 'static>(make: impl FnOnce() -> T) -> T {
    beui_core_html::document::extension_or_insert_with(make)
}

pub(crate) use beui_core_html::frame::after_input;
pub(crate) use beui_core_html::page::touched_by_finger;

pub(crate) fn on_scroll_key(
    at: KeyAt,
    phase: KeyPhase,
    handler: impl Fn(KeyEvent) -> bool + 'static,
) {
    on_key(at, phase, handler);
}

pub(crate) fn on_back(
    _enabled: Prop<bool>,
    _handler: impl Fn(crate::types::BackGesture) + 'static,
) {
}

pub(crate) fn use_keyboard() -> ReadSignal<bool> {
    create_signal(true).read_only()
}

pub(crate) fn request_paste() {}

pub(crate) fn keep_clear(_node: NodeId, _rects: Prop<Vec<crate::types::Rect>>) {}

pub(crate) fn above_embeds(_node: NodeId, _above: Prop<Option<crate::types::AboveEmbeds>>) {}

pub(crate) fn window_point(
    _pos: crate::types::Pos2,
    _from: crate::types::WindowId,
    _to: crate::types::WindowId,
) -> Option<crate::types::Pos2> {
    None
}

pub(crate) fn arrow_focus(row: NodeRef, direction: Prop<crate::types::Direction>) {
    on_key(KeyAt::Node(row), KeyPhase::Bubble, move |event| {
        let Some(press) = event.press() else {
            return false;
        };
        let (back, forth) = match direction.get_untracked() {
            crate::types::Direction::Horizontal => {
                (crate::types::Key::ArrowLeft, crate::types::Key::ArrowRight)
            }
            crate::types::Direction::Vertical => {
                (crate::types::Key::ArrowUp, crate::types::Key::ArrowDown)
            }
        };
        if press.modifiers.ctrl || press.modifiers.alt || press.modifiers.shift {
            return false;
        }
        let Some(row) = row.try_get() else {
            return false;
        };
        let stops = beui_core_html::nodes::focusables_within(row);
        let Some(at) =
            focused_node().and_then(|focused| stops.iter().position(|stop| *stop == focused))
        else {
            return false;
        };
        let to = match press.key {
            key if key == back => at.checked_sub(1),
            key if key == forth => Some(at + 1).filter(|to| *to < stops.len()),
            crate::types::Key::Home => Some(0),
            crate::types::Key::End => Some(stops.len() - 1),
            _ => None,
        };
        let Some(to) = to else {
            return false;
        };
        if press.pressed {
            set_focus(Some(stops[to]));
        }
        true
    });
}

pub(crate) use beui_core_html::typing::{
    close_keyboard, request_typed_paste, toggle_keyboard, type_events,
};
