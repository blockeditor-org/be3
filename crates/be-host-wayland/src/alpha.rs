use smithay::delegate_alpha_modifier;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::alpha_modifier::{AlphaModifierState, AlphaModifierSurfaceCachedState};
use smithay::wayland::compositor::SurfaceData;

use crate::state::State;

pub(crate) fn create(handle: &DisplayHandle) {
    AlphaModifierState::new::<State>(handle);
}

pub(crate) fn alpha(states: &SurfaceData) -> f32 {
    states
        .cached_state
        .get::<AlphaModifierSurfaceCachedState>()
        .current()
        .multiplier_f32()
        .unwrap_or(1.0)
}

delegate_alpha_modifier!(State);
