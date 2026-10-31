use std::cell::Cell;
use std::rc::Rc;

use crate::types::{BackGesture, Direction, KeyAt, KeyEvent, KeyPhase, NodeId, NodeRef};
use beui_tree::reactive::*;

use beui_core::document::Document;
use beui_core::geometry::Vec2;
use beui_core::input::Key;

pub(crate) use beui_core::timer::{Timer, create_timer, now, pin_wall_clock, wall_clock};

pub(crate) use beui_core::current::{
    close_keyboard, request_typed_paste, toggle_keyboard, try_with_document, type_events,
    with_document,
};

pub(crate) fn copy_text(text: String) {
    with_document(|document| document.copy_text(text));
}

pub(crate) fn copy_image(png: Vec<u8>) {
    with_document(|document| document.copy_image(png));
}

pub(crate) fn request_paste() {
    with_document(Document::request_paste);
}

pub(crate) fn pixels_per_point() -> ReadSignal<f32> {
    with_document(|document| document.watch_pixels_per_point())
}

pub(crate) fn use_viewport_size() -> ReadSignal<Vec2> {
    with_document(|document| document.watch_viewport_size())
}

pub(crate) fn use_fullscreen() -> ReadSignal<bool> {
    with_document(|document| document.watch_fullscreen())
}

pub(crate) fn touched_by_finger() -> bool {
    try_with_document(|document| document.touched_by_finger()).unwrap_or(false)
}

pub(crate) fn use_keyboard() -> ReadSignal<bool> {
    with_document(|document| document.watch_keyboard())
}

pub(crate) fn focus_ring(focused: Prop<bool>) -> Memo<bool> {
    let visible = with_document(|document| document.watch_focus_visible());
    let keyboard = with_document(|document| document.watch_keyboard());
    create_memo(move || focused.get() && visible.get() && keyboard.get())
}

pub(crate) fn holds_focus(node: NodeRef) -> Memo<bool> {
    let focused = with_document(|document| document.watch_focused());
    create_memo(move || {
        focused.get().is_some()
            && with_document(|document| {
                node.try_get()
                    .is_some_and(|node| document.focus_is_within(node))
            })
    })
}

pub(crate) fn on_key(at: KeyAt, phase: KeyPhase, handler: impl Fn(KeyEvent) -> bool + 'static) {
    listen(at, phase, true, handler);
}

pub(crate) fn on_back(enabled: Prop<bool>, handler: impl Fn(BackGesture) + 'static) {
    let at =
        beui_tree::reactive::try_current_component().map(|_| beui_core::tree::component_node());
    let handler = beui_core::callback::Callback::new(handler);
    let id = with_document(|document| document.add_back_handler(at, handler));
    create_binding(move || {
        let enabled = enabled.get();
        with_document(|document| document.set_back_handler_enabled(id, enabled));
    });
    on_cleanup(move || {
        try_with_document(|document| document.remove_back_handler(id));
    });
}

pub(crate) fn on_scroll_key(
    at: KeyAt,
    phase: KeyPhase,
    handler: impl Fn(KeyEvent) -> bool + 'static,
) {
    listen(at, phase, false, handler);
}

fn listen(at: KeyAt, phase: KeyPhase, reveal: bool, handler: impl Fn(KeyEvent) -> bool + 'static) {
    let handler: Rc<beui_core::keys::KeyHandler> = Rc::new(handler);
    let window = crate::window::use_window().map(|window| window.id);
    with_document(|document| {
        document.listen_keys(at, phase, reveal, Rc::downgrade(&handler), window)
    });
    on_cleanup(move || drop(handler));
}

pub(crate) fn keep_clear(node: NodeId, rects: Prop<Vec<crate::types::Rect>>) {
    create_effect(move || {
        let rects = rects.get();
        with_document(|document| document.set_kept_clear(node, Some(rects)));
    });
    on_cleanup(move || {
        try_with_document(|document| document.set_kept_clear(node, None));
    });
}

pub(crate) fn above_embeds(node: NodeId, above: Prop<Option<crate::types::AboveEmbeds>>) {
    create_effect(move || {
        let above = above.get();
        with_document(|document| document.set_above_embeds(node, above));
    });
    on_cleanup(move || {
        try_with_document(|document| document.set_above_embeds(node, None));
    });
}

pub(crate) fn to_host(window: crate::types::WindowId) -> Option<crate::types::HostTransform> {
    try_with_document(|document| Some(document.context()?.layer_space(window)?.to_host)).flatten()
}

pub(crate) fn arrow_focus(row: NodeRef, direction: Prop<Direction>) {
    on_key(KeyAt::Node(row), KeyPhase::Bubble, move |event| {
        let Some(press) = event.press() else {
            return false;
        };
        let (back, forth) = match direction.get_untracked() {
            Direction::Horizontal => (Key::ArrowLeft, Key::ArrowRight),
            Direction::Vertical => (Key::ArrowUp, Key::ArrowDown),
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
    });
}

pub(crate) fn held_modifiers() -> ReadSignal<beui_core::input::Modifiers> {
    with_document(|document| document.watch_modifiers())
}

pub(crate) fn on_finger_tap(tap: impl Fn(usize) -> bool + 'static) {
    let tap: Rc<beui_core::document::FingerTap> = Rc::new(tap);
    with_document(|document| document.register_finger_tap(Rc::downgrade(&tap)));
    on_cleanup(move || drop(tap));
}

pub(crate) fn after_input(after: impl FnOnce() + 'static) {
    with_document(|document| document.after_input(after));
}

pub(crate) fn focus_takes_text() -> bool {
    with_document(|document| document.focus_takes_text())
}

pub(crate) fn bind(node: NodeId, effect: impl FnMut() + 'static) {
    let owner = with_document(|document| document.arena.owner(node));
    match owner.filter(ScopeContext::is_alive) {
        Some(owner) => owner.run(|| create_effect(effect)),
        None => create_effect(effect),
    };
}

pub(crate) fn bind_test_id(node: NodeId, test_id: Prop<String>) {
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

pub(crate) use crate::core::nodes::{
    focus_ancestry, focused_node, opener_of, set_focus, stacked_at, tree_local, use_font_revision,
    use_preference_layer, use_preferences, window_of,
};

pub(crate) fn use_preference_layers() -> Memo<beui_core::preferences::PreferenceLayers> {
    let layers = crate::core::nodes::use_preference_layers();
    create_memo(move || layers.get())
}
