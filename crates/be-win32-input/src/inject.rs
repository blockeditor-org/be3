use std::mem::size_of;

use beui_core::geometry::Vec2;
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, HWND};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TOKEN_MANDATORY_LABEL,
    TOKEN_QUERY, TokenIntegrityLevel,
};
use windows::Win32::System::StationsAndDesktops::{
    CloseDesktop, DESKTOP_CONTROL_FLAGS, DESKTOP_READOBJECTS, GetUserObjectInformationW,
    OpenInputDesktop, UOI_NAME,
};
use windows::Win32::System::Threading::{
    AttachThreadInput, GetCurrentProcess, GetCurrentProcessId, GetCurrentThreadId, OpenProcess,
    OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBD_EVENT_FLAGS, KEYBDINPUT, MOUSE_EVENT_FLAGS,
    MOUSEINPUT, SendInput, VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AllowSetForegroundWindow, BringWindowToTop, GetForegroundWindow, GetSystemMetrics,
    GetWindowThreadProcessId, HWND_TOP, IsIconic, IsWindow, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN,
    SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_RESTORE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    SetCursorPos, SetForegroundWindow, SetWindowPos, ShowWindow,
};

use crate::mapping::PixelRect;
use crate::reach::{INTEGRITY_MEDIUM, InjectError, InputDesktop, reachable};
use crate::synth::{Event, INJECTED, Synth, plan};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    Window { hwnd: u64, frame: PixelRect },
    Display { rect: PixelRect },
}

impl Target {
    pub fn frame(&self) -> PixelRect {
        match self {
            Self::Window { frame, .. } => *frame,
            Self::Display { rect } => *rect,
        }
    }
}

pub struct Injector {
    integrity: u32,
}

impl Default for Injector {
    fn default() -> Self {
        Self::new()
    }
}

// unsafe(ffi)
impl Injector {
    pub fn new() -> Self {
        let integrity = token_integrity(unsafe { GetCurrentProcess() }).unwrap_or(INTEGRITY_MEDIUM);
        Self { integrity }
    }

    pub fn integrity(&self) -> u32 {
        self.integrity
    }

    pub fn check(&self, target: Target) -> Result<(), InjectError> {
        input_desktop().reachable()?;
        if let Target::Window { hwnd, .. } = target {
            let hwnd = window(hwnd);
            if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
                return Err(InjectError::Gone);
            }
            reachable(self.integrity, window_integrity(hwnd))?;
        }
        Ok(())
    }

    pub fn inject(&self, target: Target, size: Vec2, event: &Event) -> Result<(), InjectError> {
        self.check(target)?;
        if let Target::Window { hwnd, .. } = target {
            raise(hwnd)?;
        }
        let planned = plan(virtual_desktop(), target.frame(), size, event);
        if let Some(cursor) = planned.cursor {
            unsafe { SetCursorPos(cursor.x, cursor.y) }.map_err(refused)?;
        }
        send(&planned.synths)
    }
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
pub fn input_desktop() -> InputDesktop {
    unsafe {
        let Ok(desktop) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS)
        else {
            return InputDesktop::named(None);
        };
        let mut name = [0u16; 128];
        let mut needed = 0;
        let read = GetUserObjectInformationW(
            HANDLE(desktop.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            (name.len() * size_of::<u16>()) as u32,
            Some(&mut needed),
        );
        let _ = CloseDesktop(desktop);
        if read.is_err() {
            return InputDesktop::named(None);
        }
        let end = name
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(name.len());
        InputDesktop::named(Some(&String::from_utf16_lossy(&name[..end])))
    }
}

// unsafe(ffi)
pub fn raise(hwnd: u64) -> Result<(), InjectError> {
    let hwnd = window(hwnd);
    unsafe {
        if !IsWindow(Some(hwnd)).as_bool() {
            return Err(InjectError::Gone);
        }
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        let foreground = GetForegroundWindow();
        if foreground == hwnd {
            return Ok(());
        }
        let mut process = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut process));
        let mut foreground_process = 0;
        let foreground_thread = GetWindowThreadProcessId(foreground, Some(&mut foreground_process));
        if foreground_process == GetCurrentProcessId() && process != 0 {
            let _ = AllowSetForegroundWindow(process);
        }
        let ours = GetCurrentThreadId();
        let attached = foreground_thread != 0
            && foreground_thread != ours
            && AttachThreadInput(ours, foreground_thread, true).as_bool();
        let _ = SetWindowPos(
            hwnd,
            Some(HWND_TOP),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
        );
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        if attached {
            let _ = AttachThreadInput(ours, foreground_thread, false);
        }
        match GetForegroundWindow() == hwnd {
            true => Ok(()),
            false => Err(InjectError::NotForeground),
        }
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

// unsafe(ffi)
fn window_integrity(hwnd: HWND) -> Option<u32> {
    let mut process = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut process)) };
    if process == 0 {
        return None;
    }
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process) }.ok()?;
    let level = token_integrity(handle);
    let _ = unsafe { CloseHandle(handle) };
    level
}

// unsafe(ffi)
fn token_integrity(process: HANDLE) -> Option<u32> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(process, TOKEN_QUERY, &mut token).ok()?;
        let mut length = 0;
        let _ = GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut length);
        let mut buffer = vec![0u64; (length as usize).div_ceil(size_of::<u64>()).max(1)];
        let read = GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            length,
            &mut length,
        );
        let _ = CloseHandle(token);
        read.ok()?;
        let label = &*buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>();
        let count = *GetSidSubAuthorityCount(label.Label.Sid);
        let last = u32::from(count.checked_sub(1)?);
        Some(*GetSidSubAuthority(label.Label.Sid, last))
    }
}
