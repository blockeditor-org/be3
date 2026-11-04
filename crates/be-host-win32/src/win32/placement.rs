use std::io;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetThreadDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, SW_MAXIMIZE, SW_RESTORE, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOZORDER, SetWindowPos, ShowWindowAsync,
};

use super::desktop::rect;
use super::{hwnd, other};
use crate::{Handle, window_size_for_frame};

// unsafe(ffi)
pub fn resize_window(window: Handle, width: i32, height: i32) -> io::Result<()> {
    let window = hwnd(window);
    let previous =
        unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    let resized = (|| {
        let mut outer = RECT::default();
        unsafe { GetWindowRect(window, &mut outer) }.map_err(other)?;
        let mut frame = outer;
        let read = unsafe {
            DwmGetWindowAttribute(
                window,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&mut frame as *mut RECT).cast(),
                size_of::<RECT>() as u32,
            )
        };
        if read.is_err() {
            frame = outer;
        }
        let (width, height) = window_size_for_frame(rect(outer), rect(frame), width, height);
        unsafe {
            SetWindowPos(
                window,
                None,
                0,
                0,
                width,
                height,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            )
        }
        .map_err(other)
    })();
    if !previous.0.is_null() {
        unsafe { SetThreadDpiAwarenessContext(previous) };
    }
    resized
}

// unsafe(ffi)
pub fn set_maximized(window: Handle, maximized: bool) {
    let command = match maximized {
        true => SW_MAXIMIZE,
        false => SW_RESTORE,
    };
    let _ = unsafe { ShowWindowAsync(hwnd(window), command) };
}
