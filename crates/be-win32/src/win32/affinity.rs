use std::collections::HashSet;

use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowThreadProcessId, SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
};

use super::hwnd;
use crate::{Event, Handle};

#[derive(Default)]
pub(super) struct Excluded {
    done: HashSet<Handle>,
}

impl Excluded {
    pub(super) fn windows(&mut self, own_process: u32, windows: impl IntoIterator<Item = Handle>) {
        for window in windows {
            self.window(own_process, window);
        }
    }

    pub(super) fn events(&mut self, own_process: u32, events: &[Event]) {
        for event in events {
            match *event {
                Event::Created(window) | Event::Shown(window) => {
                    self.window(own_process, window);
                }
                Event::Destroyed(window) => {
                    self.done.remove(&window);
                }
                _ => {}
            }
        }
    }

    // unsafe(ffi)
    fn window(&mut self, own_process: u32, window: Handle) {
        if self.done.contains(&window) {
            return;
        }
        let mut process_id = 0;
        let _ = unsafe { GetWindowThreadProcessId(hwnd(window), Some(&mut process_id)) };
        if process_id != own_process {
            return;
        }
        if unsafe { SetWindowDisplayAffinity(hwnd(window), WDA_EXCLUDEFROMCAPTURE) }.is_ok() {
            self.done.insert(window);
        }
    }
}
