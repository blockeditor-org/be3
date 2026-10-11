use std::sync::Mutex;

use smithay::backend::renderer::utils::{RendererSurfaceStateUserData, SurfaceView};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::{
    BufferAssignment, SurfaceAttributes, SurfaceData, is_sync_subsurface, with_states,
};

#[derive(Default)]
struct Held(Mutex<Option<SurfaceView>>);

pub(crate) fn before_commit(surface: &WlSurface) {
    let synced = is_sync_subsurface(surface);
    with_states(surface, |states| {
        let attached = matches!(
            states.cached_state.get::<SurfaceAttributes>().current().buffer,
            Some(BufferAssignment::NewBuffer(_))
        );
        states.data_map.insert_if_missing_threadsafe(Held::default);
        let mut held = states.data_map.get::<Held>().unwrap().0.lock().unwrap();
        if attached || synced {
            *held = None;
        } else if held.is_none() {
            *held = states
                .data_map
                .get::<RendererSurfaceStateUserData>()
                .and_then(|data| data.lock().unwrap().view());
        }
    });
}

pub(crate) fn shown(states: &SurfaceData, view: SurfaceView, single_pixel: bool) -> SurfaceView {
    if single_pixel {
        return view;
    }
    let held = states
        .data_map
        .get::<Held>()
        .and_then(|held| *held.0.lock().unwrap());
    match held {
        Some(held) => SurfaceView {
            offset: view.offset,
            ..held
        },
        None => view,
    }
}
