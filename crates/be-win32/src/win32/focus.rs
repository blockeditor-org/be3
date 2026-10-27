use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcessId, GetCurrentThreadId,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    HOT_KEY_MODIFIERS, RegisterHotKey, UnregisterHotKey,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, CURSOR_SHOWING, CURSORINFO, ClipCursor, GA_ROOT, GetAncestor,
    GetClipCursor, GetCursorInfo, GetForegroundWindow, GetShellWindow, GetWindowThreadProcessId,
    HWND_TOP, IsHungAppWindow, IsIconic, SW_RESTORE, SWP_ASYNCWINDOWPOS, SWP_NOMOVE, SWP_NOSIZE,
    SetCursorPos, SetForegroundWindow, SetWindowPos, ShowWindowAsync, WindowFromPoint,
};

use super::{handle, hwnd};
use crate::focus::{Under, display_point, display_target};
use crate::{
    Command, Cursor, Entry, EntryKind, FocusError, Foreground, Hotkey, HotkeyError, Point, Snapshot,
};

pub(super) const HOTKEY_ID: i32 = 0x0BE3;

// unsafe(ffi)
pub(super) fn bring_forward(window: HWND) -> bool {
    if unsafe { IsIconic(window) }.as_bool() {
        let _ = unsafe { ShowWindowAsync(window, SW_RESTORE) };
    }
    let _ = unsafe {
        SetWindowPos(
            window,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
        )
    };
    if unsafe { SetForegroundWindow(window) }.as_bool() {
        return true;
    }
    let mut process_id = 0;
    let _ = unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    let _ = unsafe { AllowSetForegroundWindow(process_id) };
    let current = unsafe { GetCurrentThreadId() };
    let foreground = unsafe { GetForegroundWindow() };
    let foreground_thread =
        if foreground.0.is_null() || unsafe { IsHungAppWindow(foreground) }.as_bool() {
            0
        } else {
            unsafe { GetWindowThreadProcessId(foreground, None) }
        };
    let attached = foreground_thread != 0
        && foreground_thread != current
        && unsafe { AttachThreadInput(current, foreground_thread, true) }.as_bool();
    let done = unsafe { SetForegroundWindow(window) }.as_bool();
    if attached {
        let _ = unsafe { AttachThreadInput(current, foreground_thread, false) };
    }
    done || unsafe { GetForegroundWindow() } == window
}

// unsafe(ffi)
fn root_at(point: Point) -> Option<HWND> {
    let hit = unsafe {
        WindowFromPoint(POINT {
            x: point.x,
            y: point.y,
        })
    };
    if hit.0.is_null() {
        return None;
    }
    let root = unsafe { GetAncestor(hit, GA_ROOT) };
    Some(if root.0.is_null() { hit } else { root })
}

// unsafe(ffi)
fn under(point: Point, own_process: u32) -> Under {
    let Some(window) = root_at(point) else {
        return Under::Nothing;
    };
    let mut process_id = 0;
    let _ = unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    match process_id == own_process {
        true => Under::Own,
        false => Under::Window(handle(window)),
    }
}

// unsafe(ffi)
pub(super) fn raise(entry: &Entry, at: Option<Point>) -> Result<(), FocusError> {
    let target = match entry.kind {
        EntryKind::Window => hwnd(entry.handle),
        EntryKind::Display => {
            let point = display_point(entry, at);
            match display_target(under(point, unsafe { GetCurrentProcessId() }))? {
                Some(window) => hwnd(window),
                None => unsafe { GetShellWindow() },
            }
        }
    };
    if target.0.is_null() {
        return Err(FocusError::ForegroundRefused);
    }
    if bring_forward(target) {
        Ok(())
    } else {
        Err(FocusError::ForegroundRefused)
    }
}

// unsafe(ffi)
pub(super) fn foreground(snapshot: &Snapshot, own_process: u32) -> Foreground {
    if let Some(entry) = snapshot.entries.iter().find(|entry| entry.focused) {
        return Foreground::Entry(entry.id);
    }
    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return Foreground::None;
    }
    let mut process_id = 0;
    let _ = unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id == own_process {
        Foreground::Own
    } else {
        Foreground::Other
    }
}

// unsafe(ffi)
pub(super) fn apply(command: Command) {
    match command {
        Command::Clip(rect) => {
            let rect = RECT {
                left: rect.left,
                top: rect.top,
                right: rect.right,
                bottom: rect.bottom,
            };
            let _ = unsafe { ClipCursor(Some(&rect)) };
        }
        Command::Unclip => {
            let _ = unsafe { ClipCursor(None) };
        }
        Command::MoveCursor(point) => {
            let _ = unsafe { SetCursorPos(point.x, point.y) };
        }
    }
}

// unsafe(ffi)
pub(super) fn current_clip() -> Option<crate::Rect> {
    let mut rect = RECT::default();
    unsafe { GetClipCursor(&mut rect) }.ok()?;
    Some(crate::Rect::new(
        rect.left,
        rect.top,
        rect.right,
        rect.bottom,
    ))
}

// unsafe(ffi)
pub(super) fn cursor(snapshot: &Snapshot) -> Option<Cursor> {
    let mut info = CURSORINFO {
        cbSize: size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetCursorInfo(&mut info) }.ok()?;
    let POINT { x, y } = info.ptScreenPos;
    let hit = root_at(Point::new(x, y)).map(handle);
    Some(Cursor::locate(
        snapshot,
        Point::new(x, y),
        info.flags.0 & CURSOR_SHOWING.0 != 0,
        hit,
    ))
}

// unsafe(ffi)
pub(super) fn register(hotkey: Hotkey) -> Result<(), HotkeyError> {
    hotkey.check()?;
    unsafe {
        RegisterHotKey(
            None,
            HOTKEY_ID,
            HOT_KEY_MODIFIERS(hotkey.flags()),
            hotkey.key,
        )
    }
    .map_err(|error| {
        let code = error.code().0 as u32;
        let win32 = if code >> 16 == 0x8007 {
            code & 0xFFFF
        } else {
            0
        };
        hotkey.registration_error(win32, error.message())
    })
}

// unsafe(ffi)
pub(super) fn unregister() {
    let _ = unsafe { UnregisterHotKey(None, HOTKEY_ID) };
}
