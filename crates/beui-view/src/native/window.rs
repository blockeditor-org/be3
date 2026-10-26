use beui_core::window::WindowRequest;

use crate::reactive::{ReadSignal, with_document};

pub fn request_window(request: WindowRequest) -> bool {
    with_document(|document| document.request_window(request))
}

pub fn opens_windows() -> ReadSignal<bool> {
    with_document(|document| document.watch_opens_windows())
}

pub fn placed_windows() -> ReadSignal<bool> {
    with_document(|document| document.watch_placed_windows())
}
