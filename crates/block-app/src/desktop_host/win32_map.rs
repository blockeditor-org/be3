use super::win32_shape::Shaping;
use be_win32::{
    Entry, EntryId, EntryKind, FocusError, Hotkey, HotkeyError, Point, Program, Rect, Snapshot,
};
use block_plugin_api::{
    HostImage, HostProgram, HostWindow, HostWindowId, Vec2, WindowKind, WindowOrigin,
};

pub(crate) const ELEVATED: &str = "This window runs as administrator, so Block can't control it. Run Block as administrator to use it.";
pub(crate) const SECURE_DESKTOP: &str =
    "Windows is asking for permission on the secure desktop. Take off the headset to answer it.";

pub(crate) const OFF_SCREEN: &str =
    "Off screen: drag the window by its title bar to reach this part";
pub(crate) const PRESS_SLOP: f32 = 6.0;
const FALLBACK_BACK: &str = "Alt+Tab";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum HotkeyState {
    #[default]
    Waiting,
    Registered(Hotkey),
    Refused(HotkeyError),
}

impl HotkeyState {
    fn back(&self) -> String {
        match self {
            Self::Registered(hotkey) => hotkey.to_string(),
            Self::Waiting | Self::Refused(_) => FALLBACK_BACK.to_owned(),
        }
    }
}

fn this(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Window => "this window",
        EntryKind::Display => "this display",
    }
}

pub(crate) fn focused_hint(kind: EntryKind, hotkey: &HotkeyState) -> String {
    let unclipped = |why: String| {
        format!(
            "{why}, so the mouse isn't kept in {}. Press {FALLBACK_BACK} to come back to Block.",
            this(kind)
        )
    };
    match hotkey {
        HotkeyState::Registered(hotkey) => {
            format!("Windows has the mouse and keyboard. Press {hotkey} to come back to Block.")
        }
        HotkeyState::Waiting => unclipped("The shortcut back to Block isn't ready yet".to_owned()),
        HotkeyState::Refused(HotkeyError::Taken(hotkey)) => {
            unclipped(format!("{hotkey} is taken by another program"))
        }
        HotkeyState::Refused(error) => unclipped(format!(
            "The shortcut back to Block could not be set up ({error})"
        )),
    }
}

pub(crate) fn hover_hint(kind: EntryKind, hotkey: &HotkeyState) -> String {
    let mut hint = format!("Click to use {}. {} comes back.", this(kind), hotkey.back());
    if let Some(first) = hint.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    hint
}

fn subtract(rect: Rect, cut: Rect) -> Vec<Rect> {
    let Some(inside) = rect.intersection(&cut) else {
        return vec![rect];
    };
    [
        Rect::new(rect.left, rect.top, rect.right, inside.top),
        Rect::new(rect.left, inside.bottom, rect.right, rect.bottom),
        Rect::new(rect.left, inside.top, inside.left, inside.bottom),
        Rect::new(inside.right, inside.top, rect.right, inside.bottom),
    ]
    .into_iter()
    .filter(|piece| !piece.is_empty())
    .collect()
}

pub(crate) fn off_screen(frame: Rect, monitors: &[Rect]) -> Vec<Rect> {
    let mut parts = vec![frame]
        .into_iter()
        .filter(|rect| !rect.is_empty())
        .collect::<Vec<_>>();
    for monitor in monitors {
        parts = parts
            .into_iter()
            .flat_map(|part| subtract(part, *monitor))
            .collect();
    }
    parts.sort_by_key(|part| std::cmp::Reverse(i64::from(part.width()) * i64::from(part.height())));
    parts
}

pub(crate) fn off_screen_parts(snapshot: &Snapshot, entry: &Entry) -> Vec<block_plugin_api::Rect> {
    if entry.kind != EntryKind::Window || entry.minimized || entry.frame.is_empty() {
        return Vec::new();
    }
    let monitors: Vec<Rect> = snapshot
        .monitors
        .iter()
        .map(|monitor| monitor.rect)
        .collect();
    if monitors.is_empty() {
        return Vec::new();
    }
    let frame = entry.frame;
    let (width, height) = (frame.width() as f32, frame.height() as f32);
    off_screen(frame, &monitors)
        .into_iter()
        .map(|part| {
            block_plugin_api::Rect::from_min_max(
                block_plugin_api::Pos2::new(
                    (part.left - frame.left) as f32 / width,
                    (part.top - frame.top) as f32 / height,
                ),
                block_plugin_api::Pos2::new(
                    (part.right - frame.left) as f32 / width,
                    (part.bottom - frame.top) as f32 / height,
                ),
            )
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ContentPress {
    down: Option<(beui::PointerId, block_plugin_api::Pos2)>,
}

impl ContentPress {
    pub(crate) fn down(&mut self, pointer: beui::PointerId, at: block_plugin_api::Pos2) {
        self.down = Some((pointer, at));
    }

    pub(crate) fn moved(&mut self, pointer: beui::PointerId, at: block_plugin_api::Pos2) {
        if let Some((held, from)) = self.down
            && held == pointer
            && (at - from).length() > PRESS_SLOP
        {
            self.down = None;
        }
    }

    pub(crate) fn up(
        &mut self,
        pointer: beui::PointerId,
        at: block_plugin_api::Pos2,
        panel: block_plugin_api::Vec2,
    ) -> Option<block_plugin_api::Pos2> {
        self.moved(pointer, at);
        self.down.filter(|(held, _)| *held == pointer)?;
        self.down = None;
        let inside = at.x >= 0.0 && at.y >= 0.0 && at.x < panel.x && at.y < panel.y;
        inside.then_some(at)
    }

    pub(crate) fn cancel(&mut self) {
        self.down = None;
    }
}

pub(crate) fn shell_focus(_window: HostWindowId) -> Option<EntryId> {
    None
}

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

pub(crate) fn ended_notice(reason: be_win32::EndReason) -> Option<SurfaceNotice> {
    match reason {
        be_win32::EndReason::NeverArrived => {
            Some(SurfaceNotice::Focus(FocusError::ForegroundRefused))
        }
        _ => None,
    }
}

pub(crate) fn refused_notice(error: &FocusError) -> Option<SurfaceNotice> {
    match error {
        FocusError::Returned => None,
        error => Some(SurfaceNotice::Focus(error.clone())),
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
