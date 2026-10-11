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
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, request_data_device_client_selection,
    set_data_device_selection,
};
use smithay::wayland::selection::ext_data_control::{
    DataControlHandler as ExtDataControlHandler, DataControlState as ExtDataControlState,
};
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState,
};
use smithay::wayland::selection::wlr_data_control::{
    DataControlHandler as WlrDataControlHandler, DataControlState as WlrDataControlState,
};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};
use smithay::{delegate_data_control, delegate_ext_data_control, delegate_primary_selection};

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

pub(crate) struct Selection {
    data_device: DataDeviceState,
    primary: PrimarySelectionState,
    primary_on: Arc<AtomicBool>,
    ext_with_primary: ExtDataControlState,
    _ext_without_primary: ExtDataControlState,
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
        let primary_on = Arc::new(AtomicBool::new(false));
        let shown = primary_on.clone();
        let primary = PrimarySelectionState::new_with_filter::<State, _>(handle, move |_| {
            shown.load(Ordering::SeqCst)
        });
        let with = primary_on.clone();
        let without = primary_on.clone();
        let ext_with_primary =
            ExtDataControlState::new::<State, _>(handle, Some(&primary), move |_| {
                with.load(Ordering::SeqCst)
            });
        let ext_without_primary = ExtDataControlState::new::<State, _>(handle, None, move |_| {
            !without.load(Ordering::SeqCst)
        });
        let wlr = WlrDataControlState::new::<State, _>(handle, Some(&primary), |_| true);
        Self {
            data_device: DataDeviceState::new::<State>(handle),
            primary,
            primary_on,
            ext_with_primary,
            _ext_without_primary: ext_without_primary,
            wlr,
            offered: None,
            reading: None,
            read: None,
            dragging: false,
        }
    }

    pub(crate) fn set_primary(&self, on: bool) {
        self.primary_on.store(on, Ordering::SeqCst);
    }

    fn offered(&mut self, mime_types: Option<Vec<String>>) {
        self.reading = None;
        self.read = None;
        self.offered = mime_types;
    }

    pub(crate) fn take_read(&mut self) -> Option<String> {
        self.read.take()
    }

    pub(crate) fn fetch(&mut self, seat: &Seat<State>, waiter: &Waiter) {
        if let Some(mime_types) = self.offered.take() {
            self.reading = start_reading(seat, &mime_types);
        }
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
            TEXT_TYPES.iter().map(|mime| (*mime).to_owned()).collect(),
            HostText::from(text),
        );
    }
}

fn start_reading(seat: &Seat<State>, mime_types: &[String]) -> Option<Reading> {
    let mime = TEXT_TYPES
        .iter()
        .find(|text| mime_types.iter().any(|offered| offered == *text))?;
    let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).ok()?;
    let flags = rustix::fs::fcntl_getfl(&read).ok()?;
    rustix::fs::fcntl_setfl(&read, flags | OFlags::NONBLOCK).ok()?;
    request_data_device_client_selection(seat, (*mime).to_owned(), write).ok()?;
    Some(Reading {
        fd: read,
        bytes: Vec::new(),
        watched: None,
    })
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
        self.selection.set_primary(on);
    }

    pub(crate) fn fetch_selection(&mut self) {
        let waiter = self.waiter().clone();
        self.selection.fetch(&self.seat, &waiter);
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
    }
}

impl SelectionHandler for State {
    type SelectionUserData = HostText;

    fn new_selection(
        &mut self,
        target: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        if target == SelectionTarget::Clipboard {
            self.selection
                .offered(source.map(|source| source.mime_types()));
        }
    }

    fn send_selection(
        &mut self,
        _target: SelectionTarget,
        mime_type: String,
        fd: OwnedFd,
        _seat: Seat<Self>,
        text: &HostText,
    ) {
        if TEXT_TYPES.contains(&mime_type.as_str()) {
            send_text(fd, text.clone());
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
        &self.selection.ext_with_primary
    }
}

impl WlrDataControlHandler for State {
    fn data_control_state(&self) -> &WlrDataControlState {
        &self.selection.wlr
    }
}

delegate_primary_selection!(State);
delegate_data_control!(State);
delegate_ext_data_control!(State);

#[cfg(test)]
pub(crate) mod client;
#[cfg(test)]
mod tests;
