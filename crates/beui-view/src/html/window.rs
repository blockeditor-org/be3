use crate::reactive::{ReadSignal, create_signal};
use crate::types::Pos2;
use crate::types::window::{WindowId, WindowRequest};

pub(crate) fn request_window(_request: WindowRequest) -> bool {
    false
}

pub(crate) fn opens_windows() -> ReadSignal<bool> {
    create_signal(false).read_only()
}

pub(crate) fn placed_windows() -> ReadSignal<bool> {
    create_signal(false).read_only()
}

pub(crate) fn window_origin(_window: WindowId) -> Option<Pos2> {
    None
}
