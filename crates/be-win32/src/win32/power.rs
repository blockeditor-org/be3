use std::io;

use windows::Win32::Foundation::{ERROR_NOT_ALL_ASSIGNED, GetLastError, HANDLE, LUID};
use windows::Win32::Security::{
    AdjustTokenPrivileges, LUID_AND_ATTRIBUTES, LookupPrivilegeValueW, SE_PRIVILEGE_ENABLED, SE_SHUTDOWN_NAME,
    TOKEN_ADJUST_PRIVILEGES, TOKEN_PRIVILEGES, TOKEN_QUERY,
};
use windows::Win32::System::Power::{IsPwrSuspendAllowed, SetSuspendState};
use windows::Win32::System::Shutdown::{
    EWX_LOGOFF, EWX_POWEROFF, EWX_REBOOT, EXIT_WINDOWS_FLAGS, ExitWindowsEx, LockWorkStation,
    SHTDN_REASON_FLAG_PLANNED, SHTDN_REASON_MAJOR_OTHER,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::PCWSTR;

use super::{Owned, other};
use crate::{PowerAction, PowerAvailability};

// unsafe(ffi)
pub fn power_availability() -> PowerAvailability {
    PowerAvailability {
        lock: true,
        suspend: unsafe { IsPwrSuspendAllowed() },
        restart: true,
        power_off: true,
        log_out: true,
    }
}

// unsafe(ffi)
pub fn request_power(action: PowerAction) -> io::Result<()> {
    match action {
        PowerAction::Lock => unsafe { LockWorkStation() }.map_err(other),
        PowerAction::Suspend => {
            enable_shutdown_privilege()?;
            match unsafe { SetSuspendState(false, false, false) } {
                true => Ok(()),
                false => Err(io::Error::last_os_error()),
            }
        }
        PowerAction::Restart => exit_windows(EWX_REBOOT, true),
        PowerAction::PowerOff => exit_windows(EWX_POWEROFF, true),
        PowerAction::LogOut => exit_windows(EWX_LOGOFF, false),
    }
}

// unsafe(ffi)
fn exit_windows(flags: EXIT_WINDOWS_FLAGS, shutdown: bool) -> io::Result<()> {
    if shutdown {
        enable_shutdown_privilege()?;
    }
    unsafe { ExitWindowsEx(flags, SHTDN_REASON_MAJOR_OTHER | SHTDN_REASON_FLAG_PLANNED) }.map_err(other)
}

// unsafe(ffi)
fn enable_shutdown_privilege() -> io::Result<()> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES | TOKEN_QUERY, &mut token) }
        .map_err(other)?;
    let token = Owned(token);
    let mut luid = LUID::default();
    unsafe { LookupPrivilegeValueW(PCWSTR::null(), SE_SHUTDOWN_NAME, &mut luid) }.map_err(other)?;
    let privileges = TOKEN_PRIVILEGES {
        PrivilegeCount: 1,
        Privileges: [LUID_AND_ATTRIBUTES {
            Luid: luid,
            Attributes: SE_PRIVILEGE_ENABLED,
        }],
    };
    unsafe { AdjustTokenPrivileges(token.0, false, Some(&privileges), 0, None, None) }.map_err(other)?;
    if unsafe { GetLastError() } == ERROR_NOT_ALL_ASSIGNED {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "this account may not shut down the computer",
        ));
    }
    Ok(())
}
