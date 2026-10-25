use wayland_client::protocol::{
    wl_buffer, wl_seat, wl_subcompositor, wl_subsurface, wl_surface, wl_touch,
};
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::content_type::v1::client::{
    wp_content_type_manager_v1, wp_content_type_v1,
};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1, zwp_keyboard_shortcuts_inhibitor_v1,
};
use wayland_protocols::wp::single_pixel_buffer::v1::client::wp_single_pixel_buffer_manager_v1;
use wayland_protocols::wp::tablet::zv2::client::{
    zwp_tablet_manager_v2, zwp_tablet_seat_v2, zwp_tablet_tool_v2, zwp_tablet_v2,
};
use wayland_protocols::wp::tearing_control::v1::client::{
    wp_tearing_control_manager_v1, wp_tearing_control_v1,
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3, zwp_text_input_v3,
};
use wayland_protocols::wp::viewporter::client::{wp_viewport, wp_viewporter};
use wayland_protocols::xdg::foreign::zv2::client::{
    zxdg_exported_v2, zxdg_exporter_v2, zxdg_imported_v2, zxdg_importer_v2,
};
use wayland_protocols::xdg::toplevel_icon::v1::client::{
    xdg_toplevel_icon_manager_v1, xdg_toplevel_icon_v1,
};

use super::{Received, TestClient, TestWindow};

#[derive(Default)]
pub(crate) struct Protocols {
    pub(crate) wm_capabilities: Option<Vec<u32>>,
    pub(crate) inhibitor_active: Option<bool>,
    pub(crate) exported: Option<String>,
    pub(crate) icon_sizes: Vec<i32>,
    pub(crate) text_entered: bool,
    pub(crate) text_committed: Vec<String>,
    pub(crate) pen: Vec<PenSeen>,
    pub(crate) touch: Vec<TouchSeen>,
    pub(crate) seat_capabilities: Option<wl_seat::Capability>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TouchSeen {
    Down(wl_surface::WlSurface, i32, f64, f64),
    Motion(i32, f64, f64),
    Up(i32),
    Frame,
    Cancel,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PenSeen {
    Eraser(bool),
    In,
    Down,
    Motion(f64, f64),
    Pressure(u32),
    Tilt(f64, f64),
    Button(u32, bool),
    Up,
    Out,
}

impl TestClient {
    pub(crate) fn subsurface(
        &self,
        parent: &wl_surface::WlSurface,
        at: (i32, i32),
    ) -> (wl_surface::WlSurface, wl_subsurface::WlSubsurface) {
        let subcompositor: wl_subcompositor::WlSubcompositor = self.bind("wl_subcompositor", 1);
        let surface = self.surface();
        let subsurface = subcompositor.get_subsurface(&surface, parent, &self.handle, ());
        subsurface.set_position(at.0, at.1);
        subsurface.set_desync();
        (surface, subsurface)
    }

    pub(crate) fn inhibit_shortcuts(
        &self,
        surface: &wl_surface::WlSurface,
    ) -> zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1 {
        let manager: zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1 =
            self.bind("zwp_keyboard_shortcuts_inhibit_manager_v1", 1);
        manager.inhibit_shortcuts(surface, self.seat.as_ref().unwrap(), &self.handle, ())
    }

    pub(crate) fn text_input(&self) -> zwp_text_input_v3::ZwpTextInputV3 {
        let manager: zwp_text_input_manager_v3::ZwpTextInputManagerV3 =
            self.bind("zwp_text_input_manager_v3", 1);
        manager.get_text_input(self.seat.as_ref().unwrap(), &self.handle, ())
    }

    pub(crate) fn content_type(
        &self,
        surface: &wl_surface::WlSurface,
    ) -> wp_content_type_v1::WpContentTypeV1 {
        let manager: wp_content_type_manager_v1::WpContentTypeManagerV1 =
            self.bind("wp_content_type_manager_v1", 1);
        manager.get_surface_content_type(surface, &self.handle, ())
    }

    pub(crate) fn tearing_control(
        &self,
        surface: &wl_surface::WlSurface,
    ) -> wp_tearing_control_v1::WpTearingControlV1 {
        let manager: wp_tearing_control_manager_v1::WpTearingControlManagerV1 =
            self.bind("wp_tearing_control_manager_v1", 1);
        manager.get_tearing_control(surface, &self.handle, ())
    }

    pub(crate) fn tablet_seat(&self) -> zwp_tablet_seat_v2::ZwpTabletSeatV2 {
        let manager: zwp_tablet_manager_v2::ZwpTabletManagerV2 =
            self.bind("zwp_tablet_manager_v2", 1);
        manager.get_tablet_seat(self.seat.as_ref().unwrap(), &self.handle, ())
    }

    pub(crate) fn touch(&self) -> wl_touch::WlTouch {
        self.seat.as_ref().unwrap().get_touch(&self.handle, ())
    }

    pub(crate) fn viewport(&self, surface: &wl_surface::WlSurface) -> wp_viewport::WpViewport {
        let viewporter: wp_viewporter::WpViewporter = self.bind("wp_viewporter", 1);
        viewporter.get_viewport(surface, &self.handle, ())
    }

    pub(crate) fn single_pixel(&self, rgba: [u32; 4]) -> wl_buffer::WlBuffer {
        let manager: wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1 =
            self.bind("wp_single_pixel_buffer_manager_v1", 1);
        manager.create_u32_rgba_buffer(rgba[0], rgba[1], rgba[2], rgba[3], &self.handle, ())
    }

    pub(crate) fn export(&self, window: &TestWindow) -> zxdg_exported_v2::ZxdgExportedV2 {
        let exporter: zxdg_exporter_v2::ZxdgExporterV2 = self.bind("zxdg_exporter_v2", 1);
        exporter.export_toplevel(&window.surface, &self.handle, ())
    }

    pub(crate) fn import(&self, handle: String) -> zxdg_imported_v2::ZxdgImportedV2 {
        let importer: zxdg_importer_v2::ZxdgImporterV2 = self.bind("zxdg_importer_v2", 1);
        importer.import_toplevel(handle, &self.handle, ())
    }

    pub(crate) fn icon_manager(&self) -> xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1 {
        self.bind("xdg_toplevel_icon_manager_v1", 1)
    }

    pub(crate) fn icon_buffer(
        &mut self,
        size: i32,
        bgra: [u8; 4],
    ) -> (wl_buffer::WlBuffer, std::fs::File) {
        use std::io::Write as _;
        let stride = size * 4;
        let length = (stride * size) as usize;
        let fd = rustix::fs::memfd_create("icon", rustix::fs::MemfdFlags::CLOEXEC)
            .expect("a memfd opens");
        let mut file = std::fs::File::from(fd);
        file.write_all(&bgra.repeat(length / 4))
            .expect("the icon is written");
        let pool = self.shm.as_ref().unwrap().create_pool(
            std::os::fd::AsFd::as_fd(&file),
            length as i32,
            &self.handle,
            (),
        );
        let buffer = pool.create_buffer(
            0,
            size,
            size,
            stride,
            wayland_client::protocol::wl_shm::Format::Argb8888,
            &self.handle,
            (),
        );
        (buffer, file)
    }
}

impl Dispatch<zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1, ()>
    for Received
{
    fn event(
        state: &mut Self,
        _: &zwp_keyboard_shortcuts_inhibitor_v1::ZwpKeyboardShortcutsInhibitorV1,
        event: zwp_keyboard_shortcuts_inhibitor_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_keyboard_shortcuts_inhibitor_v1::Event::Active => {
                state.protocols.inhibitor_active = Some(true)
            }
            zwp_keyboard_shortcuts_inhibitor_v1::Event::Inactive => {
                state.protocols.inhibitor_active = Some(false)
            }
            _ => {}
        }
    }
}

impl Dispatch<zwp_text_input_v3::ZwpTextInputV3, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zwp_text_input_v3::ZwpTextInputV3,
        event: zwp_text_input_v3::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_text_input_v3::Event::Enter { .. } => state.protocols.text_entered = true,
            zwp_text_input_v3::Event::Leave { .. } => state.protocols.text_entered = false,
            zwp_text_input_v3::Event::CommitString { text } => {
                state.protocols.text_committed.extend(text);
            }
            _ => {}
        }
    }
}

impl Dispatch<zxdg_exported_v2::ZxdgExportedV2, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zxdg_exported_v2::ZxdgExportedV2,
        event: zxdg_exported_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_exported_v2::Event::Handle { handle } = event {
            state.protocols.exported = Some(handle);
        }
    }
}

impl Dispatch<xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1, ()> for Received {
    fn event(
        state: &mut Self,
        _: &xdg_toplevel_icon_manager_v1::XdgToplevelIconManagerV1,
        event: xdg_toplevel_icon_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_toplevel_icon_manager_v1::Event::IconSize { size } = event {
            state.protocols.icon_sizes.push(size);
        }
    }
}

wayland_client::delegate_noop!(Received: ignore zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1);
wayland_client::delegate_noop!(Received: ignore wp_content_type_manager_v1::WpContentTypeManagerV1);
wayland_client::delegate_noop!(Received: ignore zwp_text_input_manager_v3::ZwpTextInputManagerV3);
wayland_client::delegate_noop!(Received: ignore wp_content_type_v1::WpContentTypeV1);
wayland_client::delegate_noop!(Received: ignore wp_tearing_control_manager_v1::WpTearingControlManagerV1);
wayland_client::delegate_noop!(Received: ignore wp_tearing_control_v1::WpTearingControlV1);
wayland_client::delegate_noop!(Received: ignore wp_single_pixel_buffer_manager_v1::WpSinglePixelBufferManagerV1);
wayland_client::delegate_noop!(Received: ignore zxdg_exporter_v2::ZxdgExporterV2);
wayland_client::delegate_noop!(Received: ignore zxdg_importer_v2::ZxdgImporterV2);
wayland_client::delegate_noop!(Received: ignore zxdg_imported_v2::ZxdgImportedV2);
wayland_client::delegate_noop!(Received: ignore xdg_toplevel_icon_v1::XdgToplevelIconV1);
wayland_client::delegate_noop!(Received: ignore wl_subcompositor::WlSubcompositor);
wayland_client::delegate_noop!(Received: ignore wl_subsurface::WlSubsurface);
wayland_client::delegate_noop!(Received: ignore wp_viewporter::WpViewporter);
wayland_client::delegate_noop!(Received: ignore wp_viewport::WpViewport);

impl Dispatch<zwp_tablet_seat_v2::ZwpTabletSeatV2, ()> for Received {
    fn event(
        _: &mut Self,
        _: &zwp_tablet_seat_v2::ZwpTabletSeatV2,
        _: zwp_tablet_seat_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }

    wayland_client::event_created_child!(Received, zwp_tablet_seat_v2::ZwpTabletSeatV2, [
        zwp_tablet_seat_v2::EVT_TABLET_ADDED_OPCODE => (zwp_tablet_v2::ZwpTabletV2, ()),
        zwp_tablet_seat_v2::EVT_TOOL_ADDED_OPCODE => (zwp_tablet_tool_v2::ZwpTabletToolV2, ()),
    ]);
}

impl Dispatch<zwp_tablet_tool_v2::ZwpTabletToolV2, ()> for Received {
    fn event(
        state: &mut Self,
        _: &zwp_tablet_tool_v2::ZwpTabletToolV2,
        event: zwp_tablet_tool_v2::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wayland_client::WEnum;
        use zwp_tablet_tool_v2::{ButtonState, Event, Type};
        let seen = match event {
            Event::Type {
                tool_type: WEnum::Value(tool_type),
            } => PenSeen::Eraser(tool_type == Type::Eraser),
            Event::ProximityIn { .. } => PenSeen::In,
            Event::Down { .. } => PenSeen::Down,
            Event::Motion { x, y } => PenSeen::Motion(x, y),
            Event::Pressure { pressure } => PenSeen::Pressure(pressure),
            Event::Tilt { tilt_x, tilt_y } => PenSeen::Tilt(tilt_x, tilt_y),
            Event::Button {
                button,
                state: WEnum::Value(pressed),
                ..
            } => PenSeen::Button(button, pressed == ButtonState::Pressed),
            Event::Up => PenSeen::Up,
            Event::ProximityOut => PenSeen::Out,
            _ => return,
        };
        state.protocols.pen.push(seen);
    }
}

wayland_client::delegate_noop!(Received: ignore zwp_tablet_manager_v2::ZwpTabletManagerV2);
wayland_client::delegate_noop!(Received: ignore zwp_tablet_v2::ZwpTabletV2);

impl Dispatch<wl_seat::WlSeat, ()> for Received {
    fn event(
        state: &mut Self,
        _: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: wayland_client::WEnum::Value(capabilities),
        } = event
        {
            state.protocols.seat_capabilities = Some(capabilities);
        }
    }
}

impl Dispatch<wl_touch::WlTouch, ()> for Received {
    fn event(
        state: &mut Self,
        _: &wl_touch::WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let seen = match event {
            wl_touch::Event::Down {
                surface, id, x, y, ..
            } => TouchSeen::Down(surface, id, x, y),
            wl_touch::Event::Motion { id, x, y, .. } => TouchSeen::Motion(id, x, y),
            wl_touch::Event::Up { id, .. } => TouchSeen::Up(id),
            wl_touch::Event::Frame => TouchSeen::Frame,
            wl_touch::Event::Cancel => TouchSeen::Cancel,
            _ => return,
        };
        state.protocols.touch.push(seen);
    }
}
