use smithay::delegate_xdg_dialog;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::compositor::with_states;
use smithay::wayland::shell::xdg::dialog::{XdgDialogHandler, XdgDialogState};
use smithay::wayland::shell::xdg::{ToplevelSurface, XdgShellHandler, XdgToplevelSurfaceData};

use crate::state::{State, WindowId};
use crate::windows::WindowInfo;

pub fn create(handle: &DisplayHandle) {
    XdgDialogState::new::<State>(handle);
}

pub(crate) fn is_modal(toplevel: &ToplevelSurface) -> bool {
    with_states(toplevel.wl_surface(), |states| {
        states
            .data_map
            .get::<XdgToplevelSurfaceData>()
            .is_some_and(|data| data.lock().unwrap().modal)
    })
}

pub fn modal_dialog(windows: &[WindowInfo], id: WindowId) -> Option<WindowId> {
    let mut holder = id;
    let mut found = None;
    let mut seen = vec![id];
    while let Some(dialog) = windows
        .iter()
        .filter(|window| window.parent == Some(holder) && window.modal)
        .map(|window| window.id)
        .max()
        .filter(|dialog| !seen.contains(dialog))
    {
        seen.push(dialog);
        found = Some(dialog);
        holder = dialog;
    }
    found
}

impl XdgDialogHandler for State {
    fn modal_changed(&mut self, toplevel: ToplevelSurface, _modal: bool) {
        self.app_id_changed(toplevel);
    }
}

delegate_xdg_dialog!(State);
