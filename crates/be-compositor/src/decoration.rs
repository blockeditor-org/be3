use smithay::delegate_kde_decoration;
use smithay::delegate_xdg_decoration;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as XdgMode;
use smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration::{
    Mode as KdeMode, OrgKdeKwinServerDecoration,
};
use smithay::reexports::wayland_protocols_misc::server_decoration::server::org_kde_kwin_server_decoration_manager::Mode as KdeDefaultMode;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::kde::decoration::{KdeDecorationHandler, KdeDecorationState};
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgToplevelSurfaceData};
use smithay::wayland::shell::xdg::decoration::{XdgDecorationHandler, XdgDecorationState};

use crate::state::State;

pub struct Decorations {
    kde: KdeDecorationState,
}

impl Decorations {
    pub fn new(handle: &DisplayHandle) -> Self {
        XdgDecorationState::new::<State>(handle);
        Self {
            kde: KdeDecorationState::new::<State>(handle, KdeDefaultMode::Server),
        }
    }
}

pub fn server_side(toplevel: &ToplevelSurface) {
    toplevel.with_pending_state(|state| {
        state.decoration_mode = Some(XdgMode::ServerSide);
    });
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        server_side(&toplevel);
        if !toplevel.is_initial_configure_sent() {
            return;
        }
        with_states(toplevel.wl_surface(), |states| {
            if let Some(data) = states.data_map.get::<XdgToplevelSurfaceData>() {
                data.lock().unwrap().initial_decoration_configure_sent = false;
            }
        });
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: XdgMode) {
        XdgDecorationHandler::new_decoration(self, toplevel);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        XdgDecorationHandler::new_decoration(self, toplevel);
    }
}

impl KdeDecorationHandler for State {
    fn kde_decoration_state(&self) -> &KdeDecorationState {
        &self.decorations().kde
    }

    fn new_decoration(&mut self, _surface: &WlSurface, decoration: &OrgKdeKwinServerDecoration) {
        decoration.mode(KdeMode::Server);
    }
}

delegate_xdg_decoration!(State);
delegate_kde_decoration!(State);
