use crate::reactive::{ReadSignal, create_signal};
use crate::types::window::WindowRequest;

pub fn request_window(_request: WindowRequest) -> bool {
    false
}

pub fn opens_windows() -> ReadSignal<bool> {
    create_signal(false).read_only()
}

pub fn placed_windows() -> ReadSignal<bool> {
    create_signal(false).read_only()
}
