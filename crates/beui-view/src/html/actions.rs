use std::any::Any;
use std::rc::{Rc, Weak};

use crate::actions::RunRequested;
use crate::reactive::{KeyAt, KeyEvent, NodeId};
use crate::types::input::KeyChord;
use crate::types::{ActionGroup, KeyPhase, WindowId};

pub(crate) fn focus_ancestry() -> Vec<NodeId> {
    beui_core_html::focus::focus_ancestry()
}

pub(crate) fn key_window() -> Option<WindowId> {
    None
}

pub(crate) fn window_hosts() -> Vec<(WindowId, WindowId)> {
    Vec::new()
}

pub(crate) fn listen_keys(at: KeyAt, phase: KeyPhase, handler: Weak<dyn Fn(KeyEvent) -> bool>) {
    beui_core_html::document::in_document_scope(|| {
        beui_core_html::keys::listen(
            at,
            phase,
            Rc::new(move |event| handler.upgrade().is_some_and(|handler| handler(event))),
        );
    });
}

pub(crate) fn register(run: Rc<RunRequested>) -> Rc<dyn Any> {
    Rc::new(run)
}

pub(crate) fn publish_actions() -> impl Fn(Vec<ActionGroup>) + 'static {
    |_| {}
}

pub(crate) fn watch_focused() -> crate::reactive::ReadSignal<Option<NodeId>> {
    beui_core_html::focus::watch_focused()
}

pub(crate) fn watch_window_hosts() -> Option<crate::reactive::ReadSignal<Vec<(WindowId, WindowId)>>>
{
    None
}

pub(crate) fn intercepted_keys() -> impl Fn(Vec<KeyChord>) + 'static {
    |_| {}
}

pub(crate) fn in_document_scope(run: Box<dyn FnOnce()>) {
    beui_core_html::document::in_document_scope(run);
}
