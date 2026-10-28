use crate::reactive::{ReadSignal, create_signal};
use crate::types::window::WindowRequest;

pub(crate) fn request_window(_request: WindowRequest) -> bool {
    false
}

pub(crate) fn opens_windows() -> ReadSignal<bool> {
    create_signal(false).read_only()
}

pub(crate) fn placed_windows() -> ReadSignal<bool> {
    create_signal(false).read_only()
}

pub(crate) fn overlays_in_windows() -> bool {
    false
}

pub(crate) fn clear_window(_window: crate::types::WindowId) {}
