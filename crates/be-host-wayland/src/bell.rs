use smithay::delegate_xdg_system_bell;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::xdg_system_bell::{XdgSystemBellHandler, XdgSystemBellState};

use crate::state::{ServerEvent, State};

pub(crate) fn create(handle: &DisplayHandle) {
    XdgSystemBellState::new::<State>(handle);
}

impl XdgSystemBellHandler for State {
    fn ring(&mut self, surface: Option<WlSurface>) {
        if let Some(id) = surface.and_then(|surface| self.window_of_surface(&surface)) {
            self.events.push(ServerEvent::Rang(id));
        }
    }
}

delegate_xdg_system_bell!(State);
