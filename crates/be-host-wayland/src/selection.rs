use std::io::Write as _;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use rustix::fs::OFlags;
use rustix::io::Errno;
use smithay::input::Seat;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, clear_data_device_selection,
    current_data_device_selection_userdata, request_data_device_client_selection,
    set_data_device_selection,
};
use smithay::wayland::selection::ext_data_control::{
    DataControlHandler as ExtDataControlHandler, DataControlState as ExtDataControlState,
};
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState, clear_primary_selection,
    current_primary_selection_userdata, request_primary_client_selection, set_primary_selection,
};
use smithay::wayland::selection::wlr_data_control::{
    DataControlHandler as WlrDataControlHandler, DataControlState as WlrDataControlState,
};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};

use crate::server::Waiter;
use crate::state::State;

const TEXT_TYPES: [&str; 5] = [
    "text/plain;charset=utf-8",
    "UTF8_STRING",
    "text/plain",
    "STRING",
    "TEXT",
];
const MAX_TEXT: usize = 64 << 20;
const CHUNK: usize = 64 << 10;

pub(crate) type HostText = Arc<str>;

#[derive(Clone)]
pub enum Owner {
    Host(HostText),
    X11,
}

pub(crate) struct Selection {
    data_device: DataDeviceState,
    primary: PrimarySelectionState,
    primary_on: bool,
    ext: ExtDataControlState,
    wlr: WlrDataControlState,
    offered: Option<Vec<String>>,
    reading: Option<Reading>,
    read: Option<String>,
    dragging: bool,
}

struct Reading {
    fd: OwnedFd,
    bytes: Vec<u8>,
    watched: Option<Arc<AtomicBool>>,
}

enum Progress {
    Waiting,
    Done(Option<String>),
}

impl Selection {
    pub(crate) fn new(handle: &DisplayHandle) -> Self {
        let primary = PrimarySelectionState::new::<State>(handle);
        let ext = ExtDataControlState::new::<State, _>(handle, Some(&primary), |_| true);
        let wlr = WlrDataControlState::new::<State, _>(handle, Some(&primary), |_| true);
        Self {
            data_device: DataDeviceState::new::<State>(handle),
            primary,
            primary_on: false,
            ext,
            wlr,
            offered: None,
            reading: None,
            read: None,
            dragging: false,
        }
    }

    pub(crate) fn primary_on(&self) -> bool {
        self.primary_on
    }

    fn offered(&mut self, mime_types: Option<Vec<String>>) {
        self.reading = None;
        self.read = None;
        self.offered = mime_types;
    }

    pub(crate) fn take_read(&mut self) -> Option<String> {
        self.read.take()
    }

    fn progress(&mut self, waiter: &Waiter) {
        let Some(reading) = &mut self.reading else {
            return;
        };
        match reading.progress(waiter) {
            Progress::Waiting => {}
            Progress::Done(text) => {
                self.reading = None;
                self.read = text;
            }
        }
    }

    pub(crate) fn offer(&mut self, handle: &DisplayHandle, seat: &Seat<State>, text: String) {
        self.offered = None;
        self.reading = None;
        self.read = None;
        set_data_device_selection(
            handle,
            seat,
            text_types(),
            Owner::Host(HostText::from(text)),
        );
    }
}

fn text_types() -> Vec<String> {
    TEXT_TYPES.iter().map(|mime| (*mime).to_owned()).collect()
}

fn owned_by_x11(owner: Option<impl std::ops::Deref<Target = Owner>>) -> bool {
    owner.is_some_and(|owner| matches!(*owner, Owner::X11))
}

impl Reading {
    fn progress(&mut self, waiter: &Waiter) -> Progress {
        let mut chunk = vec![0; CHUNK];
        loop {
            match rustix::io::read(&self.fd, &mut chunk) {
                Ok(0) => {
                    let text = String::from_utf8_lossy(&self.bytes).into_owned();
                    return Progress::Done(Some(text));
                }
                Ok(count) => {
                    self.bytes.extend_from_slice(&chunk[..count]);
                    if self.bytes.len() > MAX_TEXT {
                        return Progress::Done(None);
                    }
                }
                Err(Errno::INTR) => {}
                Err(Errno::AGAIN) => {
                    self.watch(waiter);
                    return Progress::Waiting;
                }
                Err(_) => return Progress::Done(None),
            }
        }
    }

    fn watch(&mut self, waiter: &Waiter) {
        if self
            .watched
            .as_ref()
            .is_some_and(|fired| !fired.load(Ordering::SeqCst))
        {
            return;
        }
        let Ok(fd) = self.fd.as_fd().try_clone_to_owned() else {
            return;
        };
        let fired = Arc::new(AtomicBool::new(false));
        waiter.wait(fd, fired.clone());
        self.watched = Some(fired);
    }
}

fn send_text(fd: OwnedFd, text: HostText) {
    if let Ok(flags) = rustix::fs::fcntl_getfl(&fd) {
        let _ = rustix::fs::fcntl_setfl(&fd, flags - OFlags::NONBLOCK);
    }
    let _ = std::thread::Builder::new()
        .name("wayland selection".to_owned())
        .spawn(move || {
            let _ = std::fs::File::from(fd).write_all(text.as_bytes());
        });
}

impl State {
    pub fn set_primary_selection(&mut self, on: bool) {
        self.selection.primary_on = on;
        if !on {
            let handle = self.display_handle();
            clear_primary_selection(&handle, &self.seat);
            self.announce_x11_selection(SelectionTarget::Primary, None);
        }
    }

    pub(crate) fn fetch_selection(&mut self) {
        if let Some(mime_types) = self.selection.offered.take() {
            self.selection.reading = self.start_reading(&mime_types);
        }
        let waiter = self.waiter().clone();
        self.selection.progress(&waiter);
    }

    fn start_reading(&mut self, mime_types: &[String]) -> Option<Reading> {
        let mime = TEXT_TYPES
            .iter()
            .find(|text| mime_types.iter().any(|offered| offered == *text))?;
        let mime = (*mime).to_owned();
        let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).ok()?;
        let flags = rustix::fs::fcntl_getfl(&read).ok()?;
        rustix::fs::fcntl_setfl(&read, flags | OFlags::NONBLOCK).ok()?;
        if owned_by_x11(current_data_device_selection_userdata(&self.seat)) {
            if !self.read_x11_selection(SelectionTarget::Clipboard, mime, write) {
                return None;
            }
        } else {
            request_data_device_client_selection(&self.seat, mime, write).ok()?;
        }
        Some(Reading {
            fd: read,
            bytes: Vec::new(),
            watched: None,
        })
    }

    pub(crate) fn x11_selected(&mut self, target: SelectionTarget, mime_types: Vec<String>) {
        let handle = self.display_handle();
        match target {
            SelectionTarget::Clipboard => {
                set_data_device_selection(&handle, &self.seat, mime_types.clone(), Owner::X11);
                self.selection.offered(Some(mime_types));
            }
            SelectionTarget::Primary => {
                if !self.selection.primary_on {
                    return;
                }
                set_primary_selection(&handle, &self.seat, mime_types, Owner::X11);
            }
        }
        self.announce_x11_selection(target, None);
    }

    pub(crate) fn x11_cleared(&mut self, target: SelectionTarget) {
        let handle = self.display_handle();
        match target {
            SelectionTarget::Clipboard => {
                if owned_by_x11(current_data_device_selection_userdata(&self.seat)) {
                    clear_data_device_selection(&handle, &self.seat);
                    self.selection.offered(None);
                }
            }
            SelectionTarget::Primary => {
                if owned_by_x11(current_primary_selection_userdata(&self.seat)) {
                    clear_primary_selection(&handle, &self.seat);
                }
            }
        }
    }

    pub(crate) fn x11_paste(&mut self, target: SelectionTarget, mime_type: String, fd: OwnedFd) {
        let owner = match target {
            SelectionTarget::Clipboard => {
                current_data_device_selection_userdata(&self.seat).map(|owner| owner.clone())
            }
            SelectionTarget::Primary => {
                current_primary_selection_userdata(&self.seat).map(|owner| owner.clone())
            }
        };
        match owner {
            Some(Owner::Host(text)) => {
                if TEXT_TYPES.contains(&mime_type.as_str()) {
                    send_text(fd, text);
                }
            }
            Some(Owner::X11) => {}
            None => {
                let _ = match target {
                    SelectionTarget::Clipboard => {
                        request_data_device_client_selection(&self.seat, mime_type, fd).is_ok()
                    }
                    SelectionTarget::Primary => {
                        request_primary_client_selection(&self.seat, mime_type, fd).is_ok()
                    }
                };
            }
        }
    }

    pub(crate) fn dragging(&self) -> bool {
        self.selection.dragging
    }

    pub fn take_selection_text(&mut self) -> Option<String> {
        self.selection.take_read()
    }

    pub fn offer_selection_text(&mut self, text: String) {
        let handle = self.display_handle();
        self.selection.offer(&handle, &self.seat, text);
        self.announce_x11_selection(SelectionTarget::Clipboard, Some(text_types()));
    }
}

impl SelectionHandler for State {
    type SelectionUserData = Owner;

    fn new_selection(
        &mut self,
        target: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        let mime_types = source.map(|source| source.mime_types());
        if target == SelectionTarget::Clipboard {
            self.selection.offered(mime_types.clone());
        }
        self.announce_x11_selection(target, mime_types);
    }

    fn send_selection(
        &mut self,
        target: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
        _seat: Seat<Self>,
        owner: &Owner,
    ) {
        match owner {
            Owner::Host(text) => {
                if TEXT_TYPES.contains(&mime_type.as_str()) {
                    send_text(fd, text.clone());
                }
            }
            Owner::X11 => {
                self.read_x11_selection(target, mime_type, fd);
            }
        }
    }
}

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.selection.data_device
    }
}

impl ClientDndGrabHandler for State {
    fn started(
        &mut self,
        _source: Option<WlDataSource>,
        _icon: Option<WlSurface>,
        _seat: Seat<Self>,
    ) {
        self.selection.dragging = true;
    }

    fn dropped(&mut self, _target: Option<WlSurface>, _validated: bool, _seat: Seat<Self>) {
        self.selection.dragging = false;
    }
}

impl PrimarySelectionHandler for State {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.selection.primary
    }
}

impl ExtDataControlHandler for State {
    fn data_control_state(&self) -> &ExtDataControlState {
        &self.selection.ext
    }
}

impl WlrDataControlHandler for State {
    fn data_control_state(&self) -> &WlrDataControlState {
        &self.selection.wlr
    }
}

mod primary;

#[cfg(test)]
pub(crate) mod client;
#[cfg(test)]
mod tests;
