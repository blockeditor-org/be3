use std::any::Any;
use std::rc::{Rc, Weak};

use beui_core::context::Context;
use beui_core::document::ActionsHook;

use crate::actions::Describe;
use crate::reactive::{KeyAt, KeyEvent, NodeId, Set};
use crate::types::input::KeyChord;
use crate::types::{KeyPhase, WindowId};
use beui_core::current::with_document;

pub(crate) fn focus_ancestry() -> Vec<NodeId> {
    with_document(|document| document.focus_ancestry())
}

pub(crate) fn key_window() -> Option<WindowId> {
    with_document(|document| document.key_window())
}

pub(crate) fn window_hosts() -> Vec<(WindowId, WindowId)> {
    with_document(|document| document.window_hosts())
}

pub(crate) fn listen_keys(at: KeyAt, phase: KeyPhase, handler: Weak<dyn Fn(KeyEvent) -> bool>) {
    with_document(|document| document.listen_keys(at, phase, true, handler, None));
}

pub(crate) fn register(describe: Rc<Describe>) -> Rc<dyn Any> {
    let hook: Rc<ActionsHook> = Rc::new(
        move |context: &Context,
              path: &[NodeId],
              windows: &[WindowId],
              hosts: &[(WindowId, WindowId)]| {
            let (ran, groups) = describe(&context.action_requests(), path, windows, hosts);
            for ran in &ran {
                context.action_ran(ran);
            }
            for group in groups {
                context.publish_actions(group);
            }
        },
    );
    with_document(|document| document.register_actions(Rc::downgrade(&hook)));
    Rc::new(hook)
}

pub(crate) fn intercepted_keys() -> impl Fn(Vec<KeyChord>) + 'static {
    let writer = with_document(|document| document.intercepted_keys_writer());
    move |chords| writer.set(chords)
}

pub(crate) fn in_document_scope(run: Box<dyn FnOnce()>) {
    let scope = with_document(|document| document.reactive_scope().context());
    scope.run(run);
}
