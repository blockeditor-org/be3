use super::win32_shape::Shaping;
use be_win32::{Entry, EntryId, EntryKind, FocusError, Point, Program, Rect, Snapshot};
use block_plugin_api::{
    HostImage, HostProgram, HostWindow, HostWindowId, Vec2, WindowKind, WindowOrigin,
};

pub(crate) const ELEVATED: &str = "This window runs as administrator, so Block can't control it. Run Block as administrator to use it.";
pub(crate) const SECURE_DESKTOP: &str =
    "Windows is asking for permission on the secure desktop. Take off the headset to answer it.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SurfaceNotice {
    Elevated,
    SecureDesktop,
    Focus(FocusError),
}

impl SurfaceNotice {
    pub(crate) fn text(&self) -> String {
        match self {
            Self::Elevated => ELEVATED.to_owned(),
            Self::SecureDesktop => SECURE_DESKTOP.to_owned(),
            Self::Focus(error) => format!("{}.", capitalised(&error.to_string())),
        }
    }
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub(crate) fn surface_notice(
    snapshot: &Snapshot,
    entry: &Entry,
    refused: Option<&SurfaceNotice>,
) -> Option<SurfaceNotice> {
    if snapshot.secure_desktop() && entry.kind == EntryKind::Display {
        return Some(SurfaceNotice::SecureDesktop);
    }
    refused.cloned()
}

pub(crate) fn window_id(id: EntryId) -> HostWindowId {
    HostWindowId(id.0)
}

pub(crate) fn entry_id(id: HostWindowId) -> EntryId {
    EntryId(id.0)
}

pub(crate) fn scale_at(snapshot: &Snapshot, rect: Rect) -> f32 {
    let centre = rect.centre();
    snapshot
        .monitors
        .iter()
        .find(|monitor| monitor.rect.contains(centre.x, centre.y))
        .or_else(|| snapshot.monitors.iter().find(|monitor| monitor.primary))
        .map_or(1.0, |monitor| monitor.scale)
        .max(0.1)
}

pub(crate) fn host_window(snapshot: &Snapshot, entry: &Entry, shaping: &Shaping) -> HostWindow {
    let scale = scale_at(snapshot, entry.frame);
    let (width, height) = shaping.own_size(entry);
    let fullscreen = shaping.fullscreen_area(snapshot, entry);
    HostWindow {
        id: window_id(entry.id),
        title: entry.title.clone(),
        app_id: entry.app_id.clone(),
        parent: entry.parent.map(window_id),
        modal: entry.modal,
        size: Vec2::new(width as f32 / scale, height as f32 / scale),
        fullscreen,
        responding: entry.responding,
        focused: entry.focused,
        icon: entry.icon.as_ref().map(|icon| HostImage {
            width: icon.width,
            height: icon.height,
            rgba: icon.rgba.clone(),
        }),
        raised: entry.raised,
        kind: match entry.kind {
            EntryKind::Window => WindowKind::Window,
            EntryKind::Display => WindowKind::Display,
        },
        shown: entry.shown,
        origin: WindowOrigin {
            program: entry.origin.program.clone(),
            command: entry.origin.command.clone(),
            launch: entry.origin.launch,
        },
    }
}

pub(crate) fn listed(
    snapshot: &Snapshot,
    hidden: &[HostWindowId],
    shaping: &Shaping,
) -> Vec<HostWindow> {
    snapshot
        .entries
        .iter()
        .filter(|entry| !hidden.contains(&window_id(entry.id)))
        .map(|entry| {
            let mut window = host_window(snapshot, entry, shaping);
            window.parent = window.parent.filter(|parent| !hidden.contains(parent));
            window
        })
        .collect()
}

pub(crate) fn power_availability(
    available: be_win32::PowerAvailability,
) -> block_plugin_api::PowerAvailability {
    block_plugin_api::PowerAvailability {
        lock: available.lock,
        suspend: available.suspend,
        restart: available.restart,
        power_off: available.power_off,
        log_out: available.log_out,
        ending: false,
    }
}

pub(crate) fn power_action(action: block_plugin_api::PowerAction) -> be_win32::PowerAction {
    match action {
        block_plugin_api::PowerAction::Lock => be_win32::PowerAction::Lock,
        block_plugin_api::PowerAction::Suspend => be_win32::PowerAction::Suspend,
        block_plugin_api::PowerAction::Restart => be_win32::PowerAction::Restart,
        block_plugin_api::PowerAction::PowerOff => be_win32::PowerAction::PowerOff,
        block_plugin_api::PowerAction::LogOut => be_win32::PowerAction::LogOut,
    }
}

pub(crate) fn host_program(program: &Program, icon: Option<HostImage>) -> HostProgram {
    HostProgram {
        id: program.id.clone(),
        name: program.name.clone(),
        generic_name: String::new(),
        comment: program.shortcut.description.clone(),
        keywords: program.folder.iter().cloned().collect(),
        wm_class: program.shortcut.app_user_model_id.clone(),
        icon,
    }
}

pub(crate) fn frame_point(frame: Rect, panel: Vec2, local: block_plugin_api::Pos2) -> Point {
    let along = |at: f32, length: f32, pixels: i32| -> i32 {
        if length <= 0.0 || pixels <= 0 {
            return 0;
        }
        ((at / length * pixels as f32).floor() as i32).clamp(0, pixels - 1)
    };
    Point::new(
        along(local.x, panel.x, frame.width()),
        along(local.y, panel.y, frame.height()),
    )
}

pub(crate) fn popup_rect(frame: Rect, popup: Rect, panel: Vec2) -> block_plugin_api::Rect {
    let across = |length: f32, pixels: i32| match pixels > 0 {
        true => length / pixels as f32,
        false => 0.0,
    };
    let x = across(panel.x, frame.width());
    let y = across(panel.y, frame.height());
    block_plugin_api::Rect::from_min_size(
        block_plugin_api::Pos2::new(
            (popup.left - frame.left) as f32 * x,
            (popup.top - frame.top) as f32 * y,
        ),
        Vec2::new(popup.width() as f32 * x, popup.height() as f32 * y),
    )
}

pub(crate) fn popup_point(
    frame: Rect,
    popup: Rect,
    shown: Vec2,
    local: block_plugin_api::Pos2,
) -> Point {
    let at = frame_point(popup, shown, local);
    Point::new(popup.left - frame.left + at.x, popup.top - frame.top + at.y)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Recovery {
    Restart,
    Notice(String),
    Stop,
}

impl Recovery {
    pub(crate) fn notice(self) -> Option<String> {
        match self {
            Self::Notice(notice) => Some(notice),
            Self::Restart | Self::Stop => None,
        }
    }
}

pub(crate) fn recovery(failed: Option<&str>, restarted: bool, title: &str) -> Recovery {
    match failed {
        None => Recovery::Stop,
        Some(_) if !restarted => Recovery::Restart,
        Some(error) => Recovery::Notice(format!(
            "{} stopped being drawn, since capturing it failed twice: {error}",
            match title.is_empty() {
                true => "A window",
                false => title,
            }
        )),
    }
}

#[cfg(test)]
mod tests;
