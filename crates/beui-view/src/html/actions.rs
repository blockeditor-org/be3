use std::any::Any;
use std::rc::{Rc, Weak};

use crate::actions::Describe;
use crate::reactive::{KeyAt, KeyEvent, NodeId};
use crate::types::input::KeyChord;
use crate::types::{KeyPhase, WindowId};

pub fn document_registry<R: Clone + 'static>(make: impl FnOnce() -> R) -> (R, bool) {
    let mut fresh = false;
    let registry = beui_core_html::document::extension_or_insert_with(|| {
        fresh = true;
        make()
    });
    (registry, fresh)
}

pub fn focus_ancestry() -> Vec<NodeId> {
    beui_core_html::focus::focus_ancestry()
}

pub fn key_window() -> Option<WindowId> {
    None
}

pub fn window_hosts() -> Vec<(WindowId, WindowId)> {
    Vec::new()
}

pub fn listen_keys(at: KeyAt, phase: KeyPhase, handler: Weak<dyn Fn(KeyEvent) -> bool>) {
    beui_core_html::document::in_document_scope(|| {
        beui_core_html::keys::listen(
            at,
            phase,
            Rc::new(move |event| handler.upgrade().is_some_and(|handler| handler(event))),
        );
    });
}

pub fn register(describe: Rc<Describe>) -> Rc<dyn Any> {
    Rc::new(describe)
}

pub fn intercepted_keys() -> impl Fn(Vec<KeyChord>) + 'static {
    |_| {}
}

pub fn in_document_scope(run: Box<dyn FnOnce()>) {
    beui_core_html::document::in_document_scope(run);
}
