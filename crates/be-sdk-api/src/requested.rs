use crate::__args::{
    DESKTOP_EPAPER_REFRESH, DESKTOP_MEDIA_REQUEST, DESKTOP_NOTIFICATIONS_REPORT,
    DESKTOP_POWER_REQUEST, DESKTOP_PROGRAMS_REQUEST, DESKTOP_TRAY_REQUEST, DESKTOP_WIFI_REQUEST,
    DESKTOP_WINDOWS_REQUEST, DesktopEpaperRefreshArgs, DesktopMediaRequestArgs,
    DesktopNotificationsReportArgs, DesktopPowerRequestArgs, DesktopProgramsRequestArgs,
    DesktopTrayRequestArgs, DesktopWifiRequestArgs, DesktopWindowsRequestArgs,
};
use crate::ops::{MethodId, Started, decode};
use crate::{
    MediaRequest, NotificationReport, PowerAction, ProgramAction, TrayRequest, WifiRequest,
    WindowAction,
};

fn args<A: serde::de::DeserializeOwned>(started: &Started, method: MethodId) -> Option<A> {
    (started.method == method)
        .then(|| decode(&started.args))
        .flatten()
}

pub fn window(started: &Started) -> Option<WindowAction> {
    args::<DesktopWindowsRequestArgs>(started, DESKTOP_WINDOWS_REQUEST).map(|args| args.action)
}

pub fn power(started: &Started) -> Option<PowerAction> {
    args::<DesktopPowerRequestArgs>(started, DESKTOP_POWER_REQUEST).map(|args| args.action)
}

pub fn media(started: &Started) -> Option<MediaRequest> {
    args::<DesktopMediaRequestArgs>(started, DESKTOP_MEDIA_REQUEST).map(|args| args.request)
}

pub fn wifi(started: &Started) -> Option<WifiRequest> {
    args::<DesktopWifiRequestArgs>(started, DESKTOP_WIFI_REQUEST).map(|args| args.request)
}

pub fn epaper_refresh(started: &Started) -> Option<()> {
    args::<DesktopEpaperRefreshArgs>(started, DESKTOP_EPAPER_REFRESH).map(|_| ())
}

pub fn notification_report(started: &Started) -> Option<NotificationReport> {
    args::<DesktopNotificationsReportArgs>(started, DESKTOP_NOTIFICATIONS_REPORT)
        .map(|args| args.report)
}

pub fn program(started: &Started) -> Option<ProgramAction> {
    args::<DesktopProgramsRequestArgs>(started, DESKTOP_PROGRAMS_REQUEST).map(|args| args.action)
}

pub fn tray(started: &Started) -> Option<TrayRequest> {
    args::<DesktopTrayRequestArgs>(started, DESKTOP_TRAY_REQUEST).map(|args| args.request)
}
