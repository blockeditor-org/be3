mod desktop;
mod focus;
mod icons;
mod placement;
mod power;
mod programs;
mod watcher;

pub use desktop::Win32Desktop;
pub use placement::{resize_window, set_maximized};
pub use power::{power_availability, request_power};
pub use programs::{exec, launch, program_icon, start_menu_programs};
pub use watcher::Watcher;

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt as _;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_CLOSE};

use crate::Handle;

fn hwnd(handle: Handle) -> HWND {
    HWND(handle.0 as usize as *mut core::ffi::c_void)
}

fn handle(hwnd: HWND) -> Handle {
    Handle(hwnd.0 as usize as u64)
}

fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn from_wide(buffer: &[u16]) -> String {
    let end = buffer
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..end])
}

fn other(error: windows::core::Error) -> io::Error {
    io::Error::other(error)
}

struct Owned(HANDLE);

impl Drop for Owned {
    // unsafe(ffi)
    fn drop(&mut self) {
        let _ = unsafe { CloseHandle(self.0) };
    }
}

struct Com(bool);

impl Com {
    // unsafe(ffi)
    fn init() -> Self {
        let result =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
        Self(result.is_ok())
    }
}

impl Drop for Com {
    // unsafe(ffi)
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}

pub fn bring_forward(window: Handle) -> bool {
    focus::bring_forward(hwnd(window))
}

// unsafe(ffi)
pub fn close_window(window: Handle) -> io::Result<()> {
    unsafe { PostMessageW(Some(hwnd(window)), WM_CLOSE, WPARAM(0), LPARAM(0)) }.map_err(other)
}
