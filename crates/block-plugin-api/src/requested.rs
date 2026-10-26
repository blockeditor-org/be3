use crate::embed::desktop::{
    epaper, media, notifications, power, programs, tray, wifi, windows,
};
use crate::ops::Called;
use crate::{
    MediaRequest, NotificationReport, PowerAction, ProgramAction, TrayRequest, WifiRequest,
    WindowAction,
};

pub fn window(called: &Called) -> Option<WindowAction> {
    match windows::Call::accept_direct(called)? {
        windows::Call::Request { action } => Some(action),
        _ => None,
    }
}

pub fn power(called: &Called) -> Option<PowerAction> {
    match power::Call::accept_direct(called)? {
        power::Call::Request { action } => Some(action),
        _ => None,
    }
}

pub fn media(called: &Called) -> Option<MediaRequest> {
    match media::Call::accept_direct(called)? {
        media::Call::Request { request } => Some(request),
        _ => None,
    }
}

pub fn wifi(called: &Called) -> Option<WifiRequest> {
    match wifi::Call::accept_direct(called)? {
        wifi::Call::Request { request } => Some(request),
        _ => None,
    }
}

pub fn epaper_refresh(called: &Called) -> Option<()> {
    match epaper::Call::accept_direct(called)? {
        epaper::Call::Refresh {} => Some(()),
        _ => None,
    }
}

pub fn notification_report(called: &Called) -> Option<NotificationReport> {
    match notifications::Call::accept_direct(called)? {
        notifications::Call::Report { report } => Some(report),
        _ => None,
    }
}

pub fn program(called: &Called) -> Option<ProgramAction> {
    match programs::Call::accept_direct(called)? {
        programs::Call::Request { action } => Some(action),
        _ => None,
    }
}

pub fn tray(called: &Called) -> Option<TrayRequest> {
    match tray::Call::accept_direct(called)? {
        tray::Call::Request { request } => Some(request),
        _ => None,
    }
}
