use std::sync::atomic::{AtomicBool, Ordering};

use smithay::reexports::wayland_protocols::wp::tearing_control::v1::server::wp_tearing_control_manager_v1::{
    self, WpTearingControlManagerV1,
};
use smithay::reexports::wayland_protocols::wp::tearing_control::v1::server::wp_tearing_control_v1::{
    self, PresentationHint, WpTearingControlV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, WEnum, Weak,
};
use smithay::wayland::compositor::{Cacheable, with_states};

use crate::state::{State, WindowId};

const VERSION: u32 = 1;

pub(crate) fn create(handle: &DisplayHandle) {
    handle.create_global::<State, WpTearingControlManagerV1, _>(VERSION, ());
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TearingHint {
    pub tears: bool,
}

impl Cacheable for TearingHint {
    fn commit(&mut self, _handle: &DisplayHandle) -> Self {
        *self
    }

    fn merge_into(self, into: &mut Self, _handle: &DisplayHandle) {
        *into = self;
    }
}

#[derive(Default)]
struct Controlled(AtomicBool);

pub struct ControlData(Weak<WlSurface>);

impl State {
    pub fn tearing(&self, id: WindowId) -> bool {
        self.surface(id).is_some_and(|surface| {
            with_states(&surface, |states| {
                states.cached_state.get::<TearingHint>().current().tears
            })
        })
    }
}

fn set_hint(surface: &WlSurface, tears: bool) {
    with_states(surface, |states| {
        states.cached_state.get::<TearingHint>().pending().tears = tears;
    });
}

impl GlobalDispatch<WpTearingControlManagerV1, ()> for State {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<WpTearingControlManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WpTearingControlManagerV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        manager: &WpTearingControlManagerV1,
        request: wp_tearing_control_manager_v1::Request,
        _data: &(),
        _handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        let wp_tearing_control_manager_v1::Request::GetTearingControl { id, surface } = request
        else {
            return;
        };
        let taken = with_states(&surface, |states| {
            states
                .data_map
                .insert_if_missing_threadsafe(Controlled::default);
            let controlled = states.data_map.get::<Controlled>().unwrap();
            controlled.0.swap(true, Ordering::SeqCst)
        });
        if taken {
            manager.post_error(
                wp_tearing_control_manager_v1::Error::TearingControlExists,
                "the surface already has a tearing control",
            );
            return;
        }
        data_init.init(id, ControlData(surface.downgrade()));
    }
}

impl Dispatch<WpTearingControlV1, ControlData> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _control: &WpTearingControlV1,
        request: wp_tearing_control_v1::Request,
        data: &ControlData,
        _handle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let wp_tearing_control_v1::Request::SetPresentationHint { hint } = request else {
            return;
        };
        if let Ok(surface) = data.0.upgrade() {
            set_hint(&surface, matches!(hint, WEnum::Value(PresentationHint::Async)));
        }
    }

    fn destroyed(_state: &mut Self, _client: ClientId, _control: &WpTearingControlV1, data: &ControlData) {
        let Ok(surface) = data.0.upgrade() else {
            return;
        };
        set_hint(&surface, false);
        with_states(&surface, |states| {
            if let Some(controlled) = states.data_map.get::<Controlled>() {
                controlled.0.store(false, Ordering::SeqCst);
            }
        });
    }
}
