use smithay::reexports::wayland_protocols::ext::data_control::v1::server::{
    ext_data_control_device_v1::{self, ExtDataControlDeviceV1},
    ext_data_control_manager_v1::ExtDataControlManagerV1,
    ext_data_control_source_v1::ExtDataControlSourceV1,
};
use smithay::reexports::wayland_protocols::wp::primary_selection::zv1::server::{
    zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1,
    zwp_primary_selection_device_v1::{self, ZwpPrimarySelectionDeviceV1},
    zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
};
use smithay::reexports::wayland_protocols_wlr::data_control::v1::server::{
    zwlr_data_control_device_v1::{self, ZwlrDataControlDeviceV1},
    zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
    zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, Resource, delegate_dispatch,
    delegate_global_dispatch,
};
use smithay::wayland::selection::ext_data_control::{
    DataControlState as ExtDataControlState, ExtDataControlDeviceUserData,
    ExtDataControlManagerGlobalData, ExtDataControlManagerUserData, ExtDataControlSourceUserData,
};
use smithay::wayland::selection::primary_selection::{
    PrimaryDeviceManagerGlobalData, PrimaryDeviceUserData, PrimarySelectionState,
    PrimarySourceUserData,
};
use smithay::wayland::selection::wlr_data_control::{
    DataControlDeviceUserData, DataControlManagerGlobalData, DataControlManagerUserData,
    DataControlSourceUserData, DataControlState as WlrDataControlState,
};

use crate::state::State;

macro_rules! gated {
    ($device:ty, $data:ty, $state:ty, $set:path) => {
        impl Dispatch<$device, $data> for State {
            fn request(
                state: &mut State,
                client: &Client,
                resource: &$device,
                request: <$device as Resource>::Request,
                data: &$data,
                handle: &DisplayHandle,
                init: &mut DataInit<'_, State>,
            ) {
                if let $set { .. } = &request
                    && !state.selection.primary_on()
                {
                    return;
                }
                <$state as Dispatch<$device, $data, State>>::request(
                    state, client, resource, request, data, handle, init,
                );
            }

            fn destroyed(state: &mut State, client: ClientId, resource: &$device, data: &$data) {
                <$state as Dispatch<$device, $data, State>>::destroyed(
                    state, client, resource, data,
                );
            }
        }
    };
}

gated!(
    ZwpPrimarySelectionDeviceV1,
    PrimaryDeviceUserData,
    PrimarySelectionState,
    zwp_primary_selection_device_v1::Request::SetSelection
);
gated!(
    ExtDataControlDeviceV1,
    ExtDataControlDeviceUserData,
    ExtDataControlState,
    ext_data_control_device_v1::Request::SetPrimarySelection
);
gated!(
    ZwlrDataControlDeviceV1,
    DataControlDeviceUserData,
    WlrDataControlState,
    zwlr_data_control_device_v1::Request::SetPrimarySelection
);

delegate_global_dispatch!(State: [ZwpPrimarySelectionDeviceManagerV1: PrimaryDeviceManagerGlobalData] => PrimarySelectionState);
delegate_dispatch!(State: [ZwpPrimarySelectionDeviceManagerV1: ()] => PrimarySelectionState);
delegate_dispatch!(State: [ZwpPrimarySelectionSourceV1: PrimarySourceUserData] => PrimarySelectionState);

delegate_global_dispatch!(State: [ExtDataControlManagerV1: ExtDataControlManagerGlobalData] => ExtDataControlState);
delegate_dispatch!(State: [ExtDataControlManagerV1: ExtDataControlManagerUserData] => ExtDataControlState);
delegate_dispatch!(State: [ExtDataControlSourceV1: ExtDataControlSourceUserData] => ExtDataControlState);

delegate_global_dispatch!(State: [ZwlrDataControlManagerV1: DataControlManagerGlobalData] => WlrDataControlState);
delegate_dispatch!(State: [ZwlrDataControlManagerV1: DataControlManagerUserData] => WlrDataControlState);
delegate_dispatch!(State: [ZwlrDataControlSourceV1: DataControlSourceUserData] => WlrDataControlState);
