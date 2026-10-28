use beui_core::window::WindowRequest;

use crate::reactive::ReadSignal;
use beui_core::current::with_document;

pub(crate) fn request_window(request: WindowRequest) -> bool {
    with_document(|document| document.request_window(request))
}

pub(crate) fn opens_windows() -> ReadSignal<bool> {
    with_document(|document| document.watch_opens_windows())
}

pub(crate) fn placed_windows() -> ReadSignal<bool> {
    with_document(|document| document.watch_placed_windows())
}

pub(crate) fn overlays_in_windows() -> bool {
    with_document(|document| document.overlays_in_windows())
}

pub(crate) fn clear_window(window: beui_core::window::WindowId) {
    with_document(|document| {
        document.update_appearance(Some(window), |appearance| {
            appearance.clear_color = beui_core::color::Color32::TRANSPARENT;
        });
    });
}
