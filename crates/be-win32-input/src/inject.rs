use std::mem::size_of;

use be_win32::{Entry, Handle, Snapshot};
use beui_core::geometry::Vec2;
use windows::Win32::Foundation::{GetLastError, HWND};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, MOUSE_EVENT_FLAGS,
    MOUSEINPUT, SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetSystemMetrics, IsWindow, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN,
    SM_YVIRTUALSCREEN, SetCursorPos,
};

use crate::mapping::PixelRect;
use crate::reach::{InjectError, Target, target};
use crate::synth::{Event, INJECTED, Synth, plan};

// unsafe(ffi)
pub fn inject(
    snapshot: &Snapshot,
    entry: &Entry,
    size: Vec2,
    event: &Event,
) -> Result<(), InjectError> {
    let target = target(snapshot, entry)?;
    if let Target::Window { hwnd, .. } = target {
        if !unsafe { IsWindow(Some(window(hwnd))) }.as_bool() {
            return Err(InjectError::Gone);
        }
        if !be_win32::bring_forward(Handle(hwnd)) {
            return Err(InjectError::NotForeground);
        }
    }
    let planned = plan(virtual_desktop(), target.frame(), size, event);
    if let Some(cursor) = planned.cursor {
        unsafe { SetCursorPos(cursor.x, cursor.y) }.map_err(refused)?;
    }
    send(&planned.synths)
}

// unsafe(ffi)
pub fn virtual_desktop() -> PixelRect {
    unsafe {
        PixelRect::new(
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    }
}

fn window(hwnd: u64) -> HWND {
    HWND(hwnd as usize as *mut core::ffi::c_void)
}

fn refused(error: windows::core::Error) -> InjectError {
    InjectError::Refused {
        code: error.code().0 as u32,
    }
}

// unsafe(ffi)
fn send(synths: &[Synth]) -> Result<(), InjectError> {
    if synths.is_empty() {
        return Ok(());
    }
    let inputs: Vec<INPUT> = synths.iter().map(input).collect();
    let sent = unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
    match sent as usize == inputs.len() {
        true => Ok(()),
        false => Err(InjectError::Refused {
            code: unsafe { GetLastError() }.0,
        }),
    }
}

fn input(synth: &Synth) -> INPUT {
    match *synth {
        Synth::Mouse { x, y, data, flags } => INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: x,
                    dy: y,
                    mouseData: data as u32,
                    dwFlags: MOUSE_EVENT_FLAGS(flags),
                    time: 0,
                    dwExtraInfo: INJECTED,
                },
            },
        },
        Synth::Key { vk, scan, flags } => INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: scan,
                    dwFlags: KEYBD_EVENT_FLAGS(flags),
                    time: 0,
                    dwExtraInfo: INJECTED,
                },
            },
        },
    }
}
