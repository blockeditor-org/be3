use smithay::delegate_content_type;
use smithay::reexports::wayland_protocols::wp::content_type::v1::server::wp_content_type_v1::Type;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::compositor::with_states;
use smithay::wayland::content_type::{ContentTypeState, ContentTypeSurfaceCachedState};

use crate::state::{State, WindowId};

pub(crate) fn create(handle: &DisplayHandle) {
    ContentTypeState::new::<State>(handle);
}

impl State {
    pub fn content_type(&self, id: WindowId) -> Type {
        let Some(surface) = self.surface(id) else {
            return Type::None;
        };
        with_states(&surface, |states| {
            *states
                .cached_state
                .get::<ContentTypeSurfaceCachedState>()
                .current()
                .content_type()
        })
    }
}

delegate_content_type!(State);
