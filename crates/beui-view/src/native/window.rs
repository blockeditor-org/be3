use beui_types::{Pos2, Rect};
use beui_types::{WindowId, WindowRequest};

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

pub(crate) fn window_origin(window: WindowId) -> Option<Pos2> {
    with_document(|document| document.context()?.window_origin(window))
}

pub(crate) fn set_work_area(window: WindowId, area: Option<Rect>) {
    beui_core::current::try_with_document(|document| document.set_work_area(window, area));
}
