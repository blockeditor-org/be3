use std::cell::Cell;
use std::rc::Rc;

use beui_tree::reactive::*;

use crate::types::{Color32, KeyPhase, Outline};
use beui_core_html::keys::KeyHandler;

pub use beui_core_html::drawing::Draw;
pub use beui_core_html::focus::{ImeCursor, focus_ancestry, focused_node, set_focus};
pub use beui_core_html::frame::{Timer, create_timer, now, pin_wall_clock, wall_clock};
pub use beui_core_html::keys::{KeyAt, KeyEvent};
pub use beui_core_html::overlay::OverlayAnchor;
pub use beui_core_html::page::{
    pixels_per_point, use_font_revision, use_preferences, use_viewport_size,
};
pub use beui_core_html::{NodeId, NodeRef};

pub fn copy_text(text: impl Into<String>) {
    beui_core_html::page::copy_text(text.into());
}

pub fn focus_ring(focused: impl IntoProp<bool>) -> Memo<bool> {
    let focused = focused.into_prop();
    beui_core_html::focus::focus_ring(create_memo(move || focused.get()))
}

pub fn holds_focus(node: NodeRef) -> Memo<bool> {
    beui_core_html::focus::holds_focus(node)
}

pub fn focus_outline(
    focused: impl IntoProp<bool>,
    color: impl IntoProp<Color32>,
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

pub fn on_key(at: KeyAt, phase: KeyPhase, handler: impl Fn(KeyEvent) -> bool + 'static) {
    let handler: Rc<KeyHandler> = Rc::new(handler);
    beui_core_html::keys::listen(at, phase, handler);
}

pub fn on_shortcut(shortcut: impl Fn(crate::types::KeyPress) -> bool + 'static) {
    let at = match try_current_component() {
        Some(_) => KeyAt::Node(beui_core_html::tree::component_node()),
        None => KeyAt::Root,
    };
    on_key(at, KeyPhase::Capture, move |event| {
        match (event.press(), event.phase) {
            (Some(press), KeyPhase::Capture) => shortcut(press),
            _ => false,
        }
    });
}

pub fn on_global_key(handler: impl Fn(KeyEvent) -> bool + 'static) {
    on_key(KeyAt::Window, KeyPhase::Capture, handler);
}

pub fn bind(node: NodeId, effect: impl FnMut() + 'static) {
    match beui_core_html::document::owner(node).filter(ScopeContext::is_alive) {
        Some(owner) => owner.run(|| create_effect(effect)),
        None => create_effect(effect),
    };
}

pub fn bind_test_id(node: NodeId, test_id: Prop<String>) {
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

pub fn tree_local<T: Clone + 'static>(make: impl FnOnce() -> T) -> T {
    beui_core_html::document::extension_or_insert_with(make)
}
