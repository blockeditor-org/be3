use smithay::delegate_single_pixel_buffer;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::wayland::single_pixel_buffer::{SinglePixelBufferState, get_single_pixel_buffer};

use crate::state::State;

pub(crate) fn create(handle: &DisplayHandle) {
    SinglePixelBufferState::new::<State>(handle);
}

pub(crate) fn color(buffer: &WlBuffer) -> Option<([u8; 4], bool)> {
    let pixel = get_single_pixel_buffer(buffer).ok()?;
    Some((pixel.rgba8888(), !pixel.has_alpha()))
}

delegate_single_pixel_buffer!(State);
