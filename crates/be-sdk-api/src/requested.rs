use crate::__args::{
    DESKTOP_EPAPER_REFRESH, DESKTOP_INPUT_SWITCH_LAYOUT, DESKTOP_MEDIA_REQUEST,
    DESKTOP_NOTIFICATIONS_REPORT, DESKTOP_POWER_ANSWER_ENDING, DESKTOP_POWER_REQUEST,
    DESKTOP_PROGRAMS_REQUEST, DESKTOP_TRAY_REQUEST, DESKTOP_WIFI_REQUEST,
    DesktopEpaperRefreshArgs, DesktopInputSwitchLayoutArgs, DesktopMediaRequestArgs,
    DesktopNotificationsReportArgs, DesktopPowerAnswerEndingArgs, DesktopPowerRequestArgs,
    DesktopProgramsRequestArgs, DesktopTrayRequestArgs, DesktopWifiRequestArgs,
};
use crate::ops::{MethodId, Started, decode};
use crate::{
    EndingAnswer, MediaRequest, NotificationReport, PowerAction, ProgramAction, TrayRequest,
    WifiRequest,
};

fn args<A: serde::de::DeserializeOwned>(started: &Started, method: MethodId) -> Option<A> {
    (started.method == method)
        .then(|| decode(&started.args))
        .flatten()
}

pub fn power(started: &Started) -> Option<PowerAction> {
    args::<DesktopPowerRequestArgs>(started, DESKTOP_POWER_REQUEST).map(|args| args.action)
}

pub fn ending_answer(started: &Started) -> Option<EndingAnswer> {
    args::<DesktopPowerAnswerEndingArgs>(started, DESKTOP_POWER_ANSWER_ENDING)
        .map(|args| args.answer)
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

pub fn switch_layout(started: &Started) -> Option<u32> {
    args::<DesktopInputSwitchLayoutArgs>(started, DESKTOP_INPUT_SWITCH_LAYOUT)
        .map(|args| args.index)
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
