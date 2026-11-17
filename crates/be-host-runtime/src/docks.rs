//! The host's one `Docks` (be-sdk-app-host), which marks each app whose instances it changed for
//! the next frame.

use std::cell::RefCell;

use be_sdk_api::{ChildId, DockMessage, DockTabPath, DockUpdate, EditorInstanceId, MenuItem, ViewId};
pub(super) use be_sdk_app_host::docks::{DockedChild, Docks, Pick};
use uuid::Uuid;

thread_local! {
    static DOCKS: RefCell<Docks> = RefCell::new(Docks::default());
}

fn with<R>(act: impl FnOnce(&mut Docks) -> R) -> R {
    with_changed(act).0
}

fn with_changed<R>(act: impl FnOnce(&mut Docks) -> R) -> (R, bool) {
    let (result, changed) = DOCKS.with(|docks| {
        let mut docks = docks.borrow_mut();
        let result = act(&mut docks);
        (result, docks.take_changed())
    });
    let any = !changed.is_empty();
    if any {
        for app in changed {
            super::runtime::mark(&app);
        }
        be_host_base::host::request_repaint();
    }
    (result, any)
}

pub(super) fn register(instance: EditorInstanceId, app: &str, block: Option<Uuid>) {
    with(|docks| docks.register(instance, app, block));
}

pub(super) fn forget(instance: EditorInstanceId) {
    with(|docks| docks.forget(instance));
}

pub(super) fn restarted(app: &str) {
    with(|docks| docks.restarted(app));
}

pub(super) fn placed(
    placer: EditorInstanceId,
    children: &[DockedChild],
    framed: Option<DockedChild>,
) -> bool {
    with_changed(|docks| docks.placed(placer, children, framed)).1
}

pub(super) fn offscreen(placer: EditorInstanceId) -> bool {
    with_changed(|docks| docks.offscreen(placer)).1
}

pub(super) fn heard(app: &str, messages: Vec<DockMessage>) -> bool {
    with_changed(|docks| docks.heard(app, messages)).1
}

/// The app and instance that place a docked instance, and the child it is to them.
pub(super) fn placed_by(instance: EditorInstanceId) -> Option<(String, EditorInstanceId, ChildId)> {
    DOCKS.with(|docks| docks.borrow().placed_by(instance))
}

pub(super) fn docked(instance: EditorInstanceId) -> bool {
    DOCKS.with(|docks| docks.borrow().docked(instance))
}

pub(super) fn resolve(placer: EditorInstanceId, path: &DockTabPath) -> Option<(Uuid, u64)> {
    DOCKS.with(|docks| docks.borrow().resolve(placer, path))
}

pub(super) fn is_bar(instance: EditorInstanceId, view: u64) -> bool {
    DOCKS.with(|docks| docks.borrow().is_bar(instance, view))
}

pub(super) fn details(instance: EditorInstanceId) -> bool {
    with(|docks| docks.details(instance))
}

pub(super) fn take_details(app: &str) -> Vec<(EditorInstanceId, ChildId)> {
    DOCKS.with(|docks| docks.borrow_mut().take_details(app))
}

pub(super) fn set_menu(instance: EditorInstanceId, view: ViewId, items: Vec<MenuItem>) {
    with(|docks| docks.set_menu(instance, view, items));
}

pub(super) fn take_picks(app: &str) -> Vec<Pick> {
    DOCKS.with(|docks| docks.borrow_mut().take_picks(app))
}

pub(super) fn take(app: &str) -> Vec<(EditorInstanceId, DockUpdate)> {
    DOCKS.with(|docks| docks.borrow_mut().take(app))
}

