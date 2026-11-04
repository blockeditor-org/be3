use std::mem::size_of;

use be_host_win32::{Entry, Handle, Snapshot};
use beui_types::Vec2;
use windows::Win32::Foundation::{GetLastError, HWND};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyboardLayout, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT,
    MAPVK_VK_TO_VSC, MOUSE_EVENT_FLAGS, MOUSEINPUT, MapVirtualKeyExW, SendInput, VIRTUAL_KEY,
    VkKeyScanExW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetSystemMetrics, GetWindowThreadProcessId, IsWindow, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SetCursorPos,
};

use crate::mapping::PixelRect;
use crate::reach::{InjectError, Target, target};
use crate::synth::{Event, INJECTED, LayoutKey, Synth, plan};

// unsafe(ffi)
/// Injects into an entry with `SendInput` after bringing its window forward.
///
/// A key with a physical place goes by its scan code; one without (the on-screen keyboard,
/// `drive type`) goes as `KEYEVENTF_UNICODE` text, or, for a shortcut, as the virtual key that
/// types its character on the foreground window's layout (`VkKeyScanExW`); only a named key falls
/// back to its US scan code, which every layout shares.
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
        if !be_host_win32::bring_forward(Handle(hwnd)) {
            return Err(InjectError::NotForeground);
        }
    }
    let planned = plan(virtual_desktop(), target.frame(), size, event, &layout_key);
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

// unsafe(ffi)
fn layout_key(wanted: char) -> Option<LayoutKey> {
    let mut units = [0; 2];
    let [unit] = wanted.encode_utf16(&mut units) else {
        return None;
    };
    unsafe {
        let thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
        let layout = GetKeyboardLayout(thread);
        let scanned = VkKeyScanExW(*unit, layout);
        if scanned == -1 || scanned as u16 >> 8 != 0 {
            return None;
        }
        let vk = scanned as u16 & 0xFF;
        let scan = MapVirtualKeyExW(u32::from(vk), MAPVK_VK_TO_VSC, Some(layout)) as u16;
        Some(LayoutKey { vk, scan })
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
