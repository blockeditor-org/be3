use std::any::Any;
use std::rc::{Rc, Weak};

use beui_core::context::Context;
use beui_core::document::ActionsHook;

use crate::actions::RunRequested;
use crate::reactive::{KeyAt, KeyEvent, NodeId, Set};
use crate::types::input::KeyChord;
use crate::types::{ActionGroup, KeyPhase, WindowId};
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

pub(crate) fn register(run: Rc<RunRequested>) -> Rc<dyn Any> {
    let hook: Rc<ActionsHook> = Rc::new(
        move |context: &Context, path: &[NodeId], hosts: &[(WindowId, WindowId)]| {
            for ran in run(&context.action_requests(), path, hosts) {
                context.action_ran(&ran);
            }
        },
    );
    with_document(|document| document.register_actions(Rc::downgrade(&hook)));
    Rc::new(hook)
}

pub(crate) fn publish_actions() -> impl Fn(Vec<ActionGroup>) + 'static {
    let published = with_document(|document| document.published_actions());
    move |groups| published.publish(groups)
}

pub(crate) fn watch_focused() -> crate::reactive::ReadSignal<Option<NodeId>> {
    with_document(|document| document.watch_focused())
}

pub(crate) fn intercepted_keys() -> impl Fn(Vec<KeyChord>) + 'static {
    let writer = with_document(|document| document.intercepted_keys_writer());
    move |chords| writer.set(chords)
}

pub(crate) fn in_document_scope(run: Box<dyn FnOnce()>) {
    let scope = with_document(|document| document.reactive_scope().context());
    scope.run(run);
}
