use smithay::backend::input::{
    ButtonState, TabletToolCapabilities, TabletToolDescriptor, TabletToolType,
};
use smithay::input::pointer::CursorImageStatus;
use smithay::reexports::wayland_protocols::wp::tablet::zv2::server::zwp_tablet_manager_v2::{
    self, ZwpTabletManagerV2,
};
use smithay::reexports::wayland_protocols::wp::tablet::zv2::server::zwp_tablet_seat_v2::ZwpTabletSeatV2;
use smithay::reexports::wayland_protocols::wp::tablet::zv2::server::zwp_tablet_tool_v2::ZwpTabletToolV2;
use smithay::reexports::wayland_protocols::wp::tablet::zv2::server::zwp_tablet_v2::ZwpTabletV2;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, Resource, Weak, delegate_dispatch,
    delegate_global_dispatch,
};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};
use smithay::wayland::tablet_manager::{
    TabletDescriptor, TabletHandle, TabletManagerState, TabletSeatHandler, TabletSeatTrait,
    TabletSeatUserData, TabletToolHandle, TabletToolUserData, TabletUserData,
};

use crate::state::{State, WindowId};

const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;
const BTN_STYLUS: u32 = 0x14b;
const BTN_STYLUS2: u32 = 0x14c;

pub(crate) struct Tablet {
    managers: Vec<Weak<ZwpTabletManagerV2>>,
    tablet: Option<TabletHandle>,
    tool: Option<(TabletToolDescriptor, TabletToolHandle)>,
    window: Option<WindowId>,
    emulated: bool,
    down: bool,
}

impl Tablet {
    pub(crate) fn new(handle: &DisplayHandle) -> Self {
        TabletManagerState::new::<State>(handle);
        Self {
            managers: Vec::new(),
            tablet: None,
            tool: None,
            window: None,
            emulated: false,
            down: false,
        }
    }

    fn takes_tablets(&self, surface: &WlSurface) -> bool {
        self.managers.iter().any(|manager| {
            manager
                .upgrade()
                .is_ok_and(|manager| manager.id().same_client_as(&surface.id()))
        })
    }
}

impl State {
    pub fn pen_motion(
        &mut self,
        target: Option<(WindowId, Point<f64, Logical>)>,
        pressure: f64,
        tilt: (f64, f64),
        eraser: bool,
    ) {
        let focus = self.under(target);
        if !self.tablet.down {
            self.tablet.emulated = focus
                .as_ref()
                .and_then(|(_, _, under)| under.as_ref())
                .is_some_and(|(surface, _)| !self.tablet.takes_tablets(surface));
        }
        if self.tablet.emulated {
            self.pointer_motion(target);
            return;
        }
        let (tool, tablet) = self.pen_tool(eraser);
        let serial = SERIAL_COUNTER.next_serial();
        let time = self.time();
        tool.pressure(pressure);
        tool.tilt(tilt);
        match focus {
            Some((id, location, under)) => {
                self.tablet.window = under.is_some().then_some(id);
                tool.motion(location, under, &tablet, serial, time);
            }
            None => {
                self.tablet.window = None;
                tool.proximity_out(time);
            }
        }
    }

    pub fn pen_tip(&mut self, down: bool) {
        self.tablet.down = down;
        if self.tablet.emulated {
            self.pointer_button(BTN_LEFT, down);
            return;
        }
        let Some((_, tool)) = &self.tablet.tool else {
            return;
        };
        match down {
            true => tool.tip_down(SERIAL_COUNTER.next_serial(), self.time()),
            false => tool.tip_up(self.time()),
        }
    }

    pub fn pen_button(&mut self, button: u32, pressed: bool) {
        if self.tablet.emulated {
            self.pointer_button(button, pressed);
            return;
        }
        let Some((_, tool)) = &self.tablet.tool else {
            return;
        };
        let button = match button {
            BTN_RIGHT => BTN_STYLUS,
            BTN_MIDDLE => BTN_STYLUS2,
            button => button,
        };
        let state = match pressed {
            true => ButtonState::Pressed,
            false => ButtonState::Released,
        };
        tool.button(button, state, SERIAL_COUNTER.next_serial(), self.time());
    }

    pub fn pen_gone(&mut self) {
        if self.tablet.down {
            self.pen_tip(false);
        }
        if !std::mem::take(&mut self.tablet.emulated)
            && let Some((_, tool)) = &self.tablet.tool
        {
            tool.proximity_out(self.time());
        }
        self.tablet.window = None;
    }

    pub fn pen_left(&mut self, id: WindowId) {
        if self.tablet.window == Some(id) && !self.tablet.down {
            self.pen_gone();
        }
    }

    fn pen_tool(&mut self, eraser: bool) -> (TabletToolHandle, TabletHandle) {
        let handle = self.display_handle();
        let seat = self.seat.tablet_seat();
        let tablet = self
            .tablet
            .tablet
            .get_or_insert_with(|| {
                seat.add_tablet::<State>(
                    &handle,
                    &TabletDescriptor {
                        name: "Pen".to_owned(),
                        usb_id: None,
                        syspath: None,
                    },
                )
            })
            .clone();
        let tool_type = match eraser {
            true => TabletToolType::Eraser,
            false => TabletToolType::Pen,
        };
        if let Some((descriptor, tool)) = &self.tablet.tool
            && descriptor.tool_type == tool_type
        {
            return (tool.clone(), tablet);
        }
        if let Some((_, tool)) = self.tablet.tool.take() {
            tool.proximity_out(self.time());
        }
        let descriptor = TabletToolDescriptor {
            tool_type,
            hardware_serial: 0,
            hardware_id_wacom: 0,
            capabilities: TabletToolCapabilities::PRESSURE | TabletToolCapabilities::TILT,
        };
        let tool = seat.add_tool::<State>(self, &handle, &descriptor);
        self.tablet.tool = Some((descriptor, tool.clone()));
        (tool, tablet)
    }
}

impl TabletSeatHandler for State {
    fn tablet_tool_image(&mut self, _tool: &TabletToolDescriptor, image: CursorImageStatus) {
        self.cursor = image;
    }
}

impl Dispatch<ZwpTabletManagerV2, ()> for State {
    fn request(
        state: &mut Self,
        client: &Client,
        resource: &ZwpTabletManagerV2,
        request: zwp_tablet_manager_v2::Request,
        data: &(),
        handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwp_tablet_manager_v2::Request::GetTabletSeat { .. } = &request {
            let managers = &mut state.tablet.managers;
            managers.retain(|manager| manager.upgrade().is_ok());
            managers.push(resource.downgrade());
        }
        <TabletManagerState as Dispatch<ZwpTabletManagerV2, (), Self>>::request(
            state, client, resource, request, data, handle, data_init,
        );
    }
}

delegate_global_dispatch!(State: [ZwpTabletManagerV2: ()] => TabletManagerState);
delegate_dispatch!(State: [ZwpTabletSeatV2: TabletSeatUserData] => TabletManagerState);
delegate_dispatch!(State: [ZwpTabletToolV2: TabletToolUserData] => TabletManagerState);
delegate_dispatch!(State: [ZwpTabletV2: TabletUserData] => TabletManagerState);
