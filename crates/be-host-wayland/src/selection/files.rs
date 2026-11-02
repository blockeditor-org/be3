use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use smithay::input::Seat;
use smithay::input::pointer::GrabStartData;
use smithay::reexports::wayland_server::protocol::wl_data_device_manager::DndAction;
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::selection::data_device::{ServerDndGrabHandler, SourceMetadata, start_dnd};

use crate::state::State;

pub(crate) const URI_LIST: &str = "text/uri-list";
const TEXT: &str = "text/plain;charset=utf-8";
const BTN_LEFT: u32 = 0x110;

#[derive(Default)]
pub(crate) struct FileDrag {
    carrying: bool,
    files: Vec<PathBuf>,
}

impl State {
    pub fn file_hovered(&mut self) {
        if self.selection.files.carrying || self.selection.dragging {
            return;
        }
        self.selection.files = FileDrag {
            carrying: true,
            files: Vec::new(),
        };
        let pointer = self.pointer();
        let start = GrabStartData {
            focus: None,
            button: BTN_LEFT,
            location: pointer.current_location(),
        };
        let handle = self.display_handle();
        let seat = self.seat().clone();
        start_dnd(
            &handle,
            &seat,
            self,
            SERIAL_COUNTER.next_serial(),
            Some(start),
            None,
            SourceMetadata {
                mime_types: vec![URI_LIST.to_owned(), TEXT.to_owned()],
                dnd_action: DndAction::Copy,
            },
        );
        self.pointer_motion(self.pointer_target);
    }

    pub fn files_dropped(&mut self, files: Vec<PathBuf>) {
        if files.is_empty() {
            return;
        }
        self.file_hovered();
        if !self.selection.files.carrying {
            return;
        }
        self.selection.files.files = files;
        self.end_file_drag();
    }

    pub fn file_drag_cancelled(&mut self) {
        if !self.selection.files.carrying {
            return;
        }
        let target = self.pointer_target;
        self.pointer_motion(None);
        self.end_file_drag();
        self.pointer_motion(target);
    }

    fn end_file_drag(&mut self) {
        self.selection.files.carrying = false;
        let pointer = self.pointer();
        let time = self.time();
        pointer.unset_grab(self, SERIAL_COUNTER.next_serial(), time);
        pointer.frame(self);
    }
}

impl ServerDndGrabHandler for State {
    fn send(&mut self, mime_type: String, fd: OwnedFd, _seat: Seat<Self>) {
        let files = &self.selection.files.files;
        let text = match mime_type.as_str() {
            URI_LIST => files.iter().map(|file| uri(file) + "\r\n").collect(),
            TEXT => files
                .iter()
                .map(|file| file.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n"),
            _ => return,
        };
        super::send_text(fd, text.into());
    }

    fn cancelled(&mut self, _seat: Seat<Self>) {
        self.selection.files.files.clear();
    }

    fn finished(&mut self, _seat: Seat<Self>) {
        self.selection.files.files.clear();
    }
}

fn uri(path: &Path) -> String {
    let mut uri = "file://".to_owned();
    for byte in path.as_os_str().as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                uri.push(char::from(*byte));
            }
            _ => uri.push_str(&format!("%{byte:02X}")),
        }
    }
    uri
}
