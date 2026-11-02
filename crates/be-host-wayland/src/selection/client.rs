use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{Read as _, Write as _};
use std::os::fd::OwnedFd;

use wayland_client::backend::ObjectId;
use wayland_client::protocol::{
    wl_data_device, wl_data_device_manager, wl_data_offer, wl_data_source,
};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, event_created_child};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1, ext_data_control_manager_v1, ext_data_control_offer_v1,
    ext_data_control_source_v1,
};

use wayland_protocols::wp::primary_selection::zv1::client::{
    zwp_primary_selection_device_manager_v1, zwp_primary_selection_device_v1,
    zwp_primary_selection_offer_v1, zwp_primary_selection_source_v1,
};

use crate::test_client::{Received, TestClient};

pub(crate) const TEXT: &str = "text/plain;charset=utf-8";

#[derive(Clone)]
pub(crate) enum Offer {
    Data(wl_data_offer::WlDataOffer),
    Control(ext_data_control_offer_v1::ExtDataControlOfferV1),
    Primary(zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1),
}

impl Offer {
    fn id(&self) -> ObjectId {
        match self {
            Self::Data(offer) => offer.id(),
            Self::Control(offer) => offer.id(),
            Self::Primary(offer) => offer.id(),
        }
    }

    fn receive(&self, mime: &str, fd: std::os::fd::BorrowedFd<'_>) {
        match self {
            Self::Data(offer) => offer.receive(mime.to_owned(), fd),
            Self::Control(offer) => offer.receive(mime.to_owned(), fd),
            Self::Primary(offer) => offer.receive(mime.to_owned(), fd),
        }
    }
}

#[derive(Default)]
struct Clipboard {
    mime_types: HashMap<ObjectId, Vec<String>>,
    selection: Option<Offer>,
    copied: String,
    cancelled: bool,
    primary: Option<Offer>,
    control_primary: Option<Offer>,
    primary_cancelled: bool,
    entered: Option<wayland_client::protocol::wl_surface::WlSurface>,
    dropped: Option<wayland_client::protocol::wl_surface::WlSurface>,
}

thread_local! {
    static CLIPBOARD: RefCell<Clipboard> = RefCell::new(Clipboard::default());
}

pub(crate) struct DataControl {
    manager: ext_data_control_manager_v1::ExtDataControlManagerV1,
    device: ext_data_control_device_v1::ExtDataControlDeviceV1,
}

pub(crate) struct DataDevice {
    manager: wl_data_device_manager::WlDataDeviceManager,
    device: wl_data_device::WlDataDevice,
}

pub(crate) struct PrimaryDevice {
    manager: zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1,
    device: zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1,
}

impl TestClient {
    pub(crate) fn primary_device(&self) -> PrimaryDevice {
        let manager: zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1 =
            self.bind("zwp_primary_selection_device_manager_v1", 1);
        let device = manager.get_device(self.seat.as_ref().unwrap(), &self.handle, ());
        PrimaryDevice { manager, device }
    }

    pub(crate) fn select(&self, device: &PrimaryDevice, text: &str, serial: u32) {
        let source = device.manager.create_source(&self.handle, ());
        source.offer(TEXT.to_owned());
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().copied = text.to_owned());
        device.device.set_selection(Some(&source), serial);
    }

    pub(crate) fn select_with_control(&self, control: &DataControl, text: &str) {
        let source = control.manager.create_data_source(&self.handle, ());
        source.offer(TEXT.to_owned());
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().copied = text.to_owned());
        control.device.set_primary_selection(Some(&source));
    }

    pub(crate) fn data_control(&self) -> DataControl {
        let manager: ext_data_control_manager_v1::ExtDataControlManagerV1 =
            self.bind("ext_data_control_manager_v1", 1);
        let device = manager.get_data_device(self.seat.as_ref().unwrap(), &self.handle, ());
        DataControl { manager, device }
    }

    pub(crate) fn data_device(&self) -> DataDevice {
        let manager: wl_data_device_manager::WlDataDeviceManager =
            self.bind("wl_data_device_manager", 3);
        let device = manager.get_data_device(self.seat.as_ref().unwrap(), &self.handle, ());
        DataDevice { manager, device }
    }

    pub(crate) fn start_drag(
        &self,
        device: &DataDevice,
        origin: &wayland_client::protocol::wl_surface::WlSurface,
        serial: u32,
    ) {
        let source = device.manager.create_data_source(&self.handle, ());
        source.offer(TEXT.to_owned());
        source.set_actions(wl_data_device_manager::DndAction::Copy);
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().copied = "dragged".to_owned());
        device
            .device
            .start_drag(Some(&source), origin, None, serial);
    }

    pub(crate) fn copy_with_control(&self, control: &DataControl, text: &str) {
        let source = control.manager.create_data_source(&self.handle, ());
        source.offer(TEXT.to_owned());
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().copied = text.to_owned());
        control.device.set_selection(Some(&source));
    }

    pub(crate) fn copy_with_device(&self, device: &DataDevice, text: &str, serial: u32) {
        let source = device.manager.create_data_source(&self.handle, ());
        source.offer(TEXT.to_owned());
        source.offer("text/html".to_owned());
        CLIPBOARD.with(|clipboard| clipboard.borrow_mut().copied = text.to_owned());
        device.device.set_selection(Some(&source), serial);
    }
}

pub(crate) fn selection() -> Option<Offer> {
    CLIPBOARD.with(|clipboard| clipboard.borrow().selection.clone())
}

pub(crate) fn primary() -> Option<Offer> {
    CLIPBOARD.with(|clipboard| clipboard.borrow().primary.clone())
}

pub(crate) fn control_primary() -> Option<Offer> {
    CLIPBOARD.with(|clipboard| clipboard.borrow().control_primary.clone())
}

pub(crate) fn primary_cancelled() -> bool {
    CLIPBOARD.with(|clipboard| clipboard.borrow().primary_cancelled)
}

pub(crate) fn drop_target() -> Option<wayland_client::protocol::wl_surface::WlSurface> {
    CLIPBOARD.with(|clipboard| clipboard.borrow().entered.clone())
}

pub(crate) fn dropped_on() -> Option<wayland_client::protocol::wl_surface::WlSurface> {
    CLIPBOARD.with(|clipboard| clipboard.borrow().dropped.clone())
}

pub(crate) fn source_cancelled() -> bool {
    CLIPBOARD.with(|clipboard| clipboard.borrow().cancelled)
}

pub(crate) fn mime_types(offer: &Offer) -> Vec<String> {
    CLIPBOARD.with(|clipboard| {
        clipboard
            .borrow()
            .mime_types
            .get(&offer.id())
            .cloned()
            .unwrap_or_default()
    })
}

pub(crate) fn ask(offer: &Offer) -> OwnedFd {
    let (read, write) = rustix::pipe::pipe().expect("a pipe opens");
    offer.receive(TEXT, std::os::fd::AsFd::as_fd(&write));
    read
}

pub(crate) fn answer(read: OwnedFd) -> String {
    let mut text = String::new();
    std::fs::File::from(read)
        .read_to_string(&mut text)
        .expect("the answer is read");
    text
}

fn offered(offer: ObjectId, mime_type: String) {
    CLIPBOARD.with(|clipboard| {
        clipboard
            .borrow_mut()
            .mime_types
            .entry(offer)
            .or_default()
            .push(mime_type);
    });
}

fn selected(offer: Option<Offer>) {
    CLIPBOARD.with(|clipboard| {
        clipboard.borrow_mut().selection = offer;
    });
}

fn send(fd: OwnedFd) {
    let text = CLIPBOARD.with(|clipboard| clipboard.borrow().copied.clone());
    let _ = std::fs::File::from(fd).write_all(text.as_bytes());
}

impl Dispatch<wl_data_device::WlDataDevice, ()> for Received {
    fn event(
        _: &mut Self,
        _: &wl_data_device::WlDataDevice,
        event: wl_data_device::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        CLIPBOARD.with(|clipboard| {
            let mut clipboard = clipboard.borrow_mut();
            match &event {
                wl_data_device::Event::Enter {
                    surface,
                    serial,
                    id,
                    ..
                } => {
                    clipboard.entered = Some(surface.clone());
                    if let Some(offer) = id {
                        offer.accept(*serial, Some(TEXT.to_owned()));
                        offer.set_actions(
                            wl_data_device_manager::DndAction::Copy,
                            wl_data_device_manager::DndAction::Copy,
                        );
                    }
                }
                wl_data_device::Event::Leave => clipboard.entered = None,
                wl_data_device::Event::Drop => clipboard.dropped = clipboard.entered.clone(),
                _ => {}
            }
        });
        if let wl_data_device::Event::Selection { id } = event {
            selected(id.map(Offer::Data));
        }
    }

    event_created_child!(Received, wl_data_device::WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (wl_data_offer::WlDataOffer, ()),
    ]);
}

impl Dispatch<wl_data_offer::WlDataOffer, ()> for Received {
    fn event(
        _: &mut Self,
        offer: &wl_data_offer::WlDataOffer,
        event: wl_data_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_offer::Event::Offer { mime_type } = event {
            offered(offer.id(), mime_type);
        }
    }
}

impl Dispatch<wl_data_source::WlDataSource, ()> for Received {
    fn event(
        _: &mut Self,
        _: &wl_data_source::WlDataSource,
        event: wl_data_source::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_data_source::Event::Send { fd, .. } => send(fd),
            wl_data_source::Event::Cancelled => {
                CLIPBOARD.with(|clipboard| clipboard.borrow_mut().cancelled = true);
            }
            _ => {}
        }
    }
}

impl Dispatch<ext_data_control_device_v1::ExtDataControlDeviceV1, ()> for Received {
    fn event(
        _: &mut Self,
        _: &ext_data_control_device_v1::ExtDataControlDeviceV1,
        event: ext_data_control_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::Selection { id } => {
                selected(id.map(Offer::Control));
            }
            ext_data_control_device_v1::Event::PrimarySelection { id } => {
                CLIPBOARD.with(|clipboard| {
                    clipboard.borrow_mut().control_primary = id.map(Offer::Control);
                });
            }
            _ => {}
        }
    }

    event_created_child!(Received, ext_data_control_device_v1::ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ext_data_control_offer_v1::ExtDataControlOfferV1, ()),
    ]);
}

impl Dispatch<ext_data_control_offer_v1::ExtDataControlOfferV1, ()> for Received {
    fn event(
        _: &mut Self,
        offer: &ext_data_control_offer_v1::ExtDataControlOfferV1,
        event: ext_data_control_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            offered(offer.id(), mime_type);
        }
    }
}

impl Dispatch<ext_data_control_source_v1::ExtDataControlSourceV1, ()> for Received {
    fn event(
        _: &mut Self,
        _: &ext_data_control_source_v1::ExtDataControlSourceV1,
        event: ext_data_control_source_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_source_v1::Event::Send { fd, .. } => send(fd),
            ext_data_control_source_v1::Event::Cancelled => {
                CLIPBOARD.with(|clipboard| clipboard.borrow_mut().cancelled = true);
            }
            _ => {}
        }
    }
}

impl Dispatch<zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1, ()> for Received {
    fn event(
        _: &mut Self,
        _: &zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1,
        event: zwp_primary_selection_device_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwp_primary_selection_device_v1::Event::Selection { id } = event {
            CLIPBOARD.with(|clipboard| clipboard.borrow_mut().primary = id.map(Offer::Primary));
        }
    }

    event_created_child!(Received, zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1, [
        zwp_primary_selection_device_v1::EVT_DATA_OFFER_OPCODE => (zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1, ()),
    ]);
}

impl Dispatch<zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1, ()> for Received {
    fn event(
        _: &mut Self,
        offer: &zwp_primary_selection_offer_v1::ZwpPrimarySelectionOfferV1,
        event: zwp_primary_selection_offer_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwp_primary_selection_offer_v1::Event::Offer { mime_type } = event {
            offered(offer.id(), mime_type);
        }
    }
}

impl Dispatch<zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1, ()> for Received {
    fn event(
        _: &mut Self,
        _: &zwp_primary_selection_source_v1::ZwpPrimarySelectionSourceV1,
        event: zwp_primary_selection_source_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_primary_selection_source_v1::Event::Send { fd, .. } => send(fd),
            zwp_primary_selection_source_v1::Event::Cancelled => {
                CLIPBOARD.with(|clipboard| clipboard.borrow_mut().primary_cancelled = true);
            }
            _ => {}
        }
    }
}

wayland_client::delegate_noop!(Received: ignore zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1);

wayland_client::delegate_noop!(Received: ignore wl_data_device_manager::WlDataDeviceManager);
wayland_client::delegate_noop!(Received: ignore ext_data_control_manager_v1::ExtDataControlManagerV1);
