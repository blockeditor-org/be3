use std::cell::{Cell, RefCell};

use block_plugin_api::{EditorInstanceId, WindowRequest};

type Overlays = fn(&str, EditorInstanceId, u64) -> bool;

thread_local! {
    static PENDING: RefCell<Vec<(String, WindowRequest)>> = const { RefCell::new(Vec::new()) };
    static FORGOTTEN: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static OVERLAYS: Cell<Option<Overlays>> = const { Cell::new(None) };
}

pub(crate) fn request_windows(plugin: &str, requests: Vec<WindowRequest>) {
    if requests.is_empty() {
        return;
    }
    PENDING.with(|pending| {
        pending.borrow_mut().extend(
            requests
                .into_iter()
                .map(|request| (plugin.to_owned(), request)),
        )
    });
    block_app_base::host::request_repaint();
}

pub(crate) fn forget_windows(plugin: &str) {
    FORGOTTEN.with(|forgotten| forgotten.borrow_mut().push(plugin.to_owned()));
    block_app_base::host::request_repaint();
}

pub(crate) fn take_window_requests() -> (Vec<(String, WindowRequest)>, Vec<String>) {
    (
        PENDING.with(|pending| std::mem::take(&mut *pending.borrow_mut())),
        FORGOTTEN.with(|forgotten| std::mem::take(&mut *forgotten.borrow_mut())),
    )
}

pub(crate) fn serve_overlays(overlays: Overlays) {
    OVERLAYS.with(|held| held.set(Some(overlays)));
}

pub(crate) fn is_overlay(plugin: &str, instance: EditorInstanceId, window: u64) -> bool {
    OVERLAYS
        .with(Cell::get)
        .is_some_and(|overlays| overlays(plugin, instance, window))
}
