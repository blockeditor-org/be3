use std::collections::HashMap;

use smithay::delegate_foreign_toplevel_list;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::foreign_toplevel_list::{
    ForeignToplevelHandle, ForeignToplevelListHandler, ForeignToplevelListState,
};

use crate::state::{State, WindowId};

pub(crate) struct ToplevelList {
    state: ForeignToplevelListState,
    handles: HashMap<WindowId, ForeignToplevelHandle>,
}

impl ToplevelList {
    pub(crate) fn new(handle: &DisplayHandle) -> Self {
        Self {
            state: ForeignToplevelListState::new::<State>(handle),
            handles: HashMap::new(),
        }
    }
}

impl State {
    pub(crate) fn publish_toplevels(&mut self) {
        let listed: Vec<(WindowId, String, String)> = self
            .windows
            .iter()
            .map(|window| window.id)
            .filter(|id| self.mapped_size(*id).is_some())
            .map(|id| {
                let title = self.title(id).unwrap_or_default();
                (id, title, self.program_id(id).unwrap_or_default())
            })
            .collect();
        let ToplevelList { state, handles } = &mut self.toplevel_list;
        handles.retain(|id, handle| {
            let open = listed.iter().any(|(listed, _, _)| listed == id);
            if !open {
                state.remove_toplevel(handle);
            }
            open
        });
        for (id, title, app_id) in listed {
            match handles.get(&id) {
                Some(handle) => {
                    if handle.title() != title || handle.app_id() != app_id {
                        handle.send_title(&title);
                        handle.send_app_id(&app_id);
                        handle.send_done();
                    }
                }
                None => {
                    handles.insert(id, state.new_toplevel::<State>(title, app_id));
                }
            }
        }
    }
}

impl ForeignToplevelListHandler for State {
    fn foreign_toplevel_list_state(&mut self) -> &mut ForeignToplevelListState {
        &mut self.toplevel_list.state
    }
}

delegate_foreign_toplevel_list!(State);
