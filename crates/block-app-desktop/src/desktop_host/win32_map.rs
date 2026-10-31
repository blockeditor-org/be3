use std::collections::HashMap;

use super::win32_shape::Shaping;
use be_win32::{
    Entry, EntryId, EntryKind, FocusError, Hotkey, HotkeyError, Point, Program, Rect, Snapshot,
};
use block_plugin_api::{
    HostImage, HostProgram, HostWindow, HostWindowId, Vec2, WindowKind, WindowOrigin,
};

pub const ELEVATED: &str = "This window runs as administrator, so Block can't control it. Run Block as administrator to use it.";
pub const SECURE_DESKTOP: &str =
    "Windows is asking for permission on the secure desktop. Take off the headset to answer it.";

pub const OFF_SCREEN: &str = "Off screen: drag the window by its title bar to reach this part";
pub const PRESS_SLOP: f32 = 6.0;
const FALLBACK_BACK: &str = "Alt+Tab";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HotkeyState {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    Elevated,
    Focus(FocusError),
}

impl Refusal {
    pub fn text(&self) -> String {
        match self {
            Self::Elevated => ELEVATED.to_owned(),
            Self::Focus(error) => format!("{}.", capitalised(&error.to_string())),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notes {
    pub focused: Option<EntryId>,
    pub hovered: Option<EntryId>,
    pub refused: HashMap<EntryId, Refusal>,
    pub hotkey: HotkeyState,
}

impl Notes {
    pub fn note(&self, entry: &Entry) -> Option<String> {
        if self.focused == Some(entry.id) {
            return Some(focused_note(&self.hotkey));
        }
        if let Some(refusal) = self.refused.get(&entry.id) {
            return Some(refusal.text());
        }
        (self.hovered == Some(entry.id)).then(|| hover_note(entry.kind, &self.hotkey))
    }
}

pub fn focused_note(hotkey: &HotkeyState) -> String {
    let unclipped = |why: String| {
        format!("{FALLBACK_BACK} comes back to Block. The mouse is free, since {why}")
    };
    match hotkey {
        HotkeyState::Registered(hotkey) => format!("{hotkey} comes back to Block"),
        HotkeyState::Waiting => unclipped("the shortcut back isn't ready yet".to_owned()),
        HotkeyState::Refused(HotkeyError::Taken(hotkey)) => {
            unclipped(format!("{hotkey} is taken by another program"))
        }
        HotkeyState::Refused(error) => {
            unclipped(format!("the shortcut back could not be set up ({error})"))
        }
    }
}

pub fn hover_note(kind: EntryKind, hotkey: &HotkeyState) -> String {
    let what = match kind {
        EntryKind::Window => "this window",
        EntryKind::Display => "the window under the pointer",
    };
    format!("Click to use {what}. {} comes back to Block", hotkey.back())
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

pub fn off_screen(frame: Rect, monitors: &[Rect]) -> Vec<Rect> {
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

pub fn off_screen_parts(snapshot: &Snapshot, entry: &Entry) -> Vec<block_plugin_api::Rect> {
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
pub struct ContentPress {
    down: Option<(beui::PointerId, block_plugin_api::Pos2)>,
}

impl ContentPress {
    pub fn down(&mut self, pointer: beui::PointerId, at: block_plugin_api::Pos2) {
        self.down = Some((pointer, at));
    }

    pub fn moved(&mut self, pointer: beui::PointerId, at: block_plugin_api::Pos2) {
        if let Some((held, from)) = self.down
            && held == pointer
            && (at - from).length() > PRESS_SLOP
        {
            self.down = None;
        }
    }

    pub fn up(
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

    pub fn cancel(&mut self) {
        self.down = None;
    }
}

pub fn shell_focus(_window: HostWindowId) -> Option<EntryId> {
    None
}

fn capitalised(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub fn ended_refusal(reason: be_win32::EndReason) -> Option<Refusal> {
    match reason {
        be_win32::EndReason::NeverArrived => Some(Refusal::Focus(FocusError::ForegroundRefused)),
        _ => None,
    }
}

pub fn focus_refusal(error: &FocusError) -> Option<Refusal> {
    match error {
        FocusError::Returned | FocusError::OnBlock => None,
        error => Some(Refusal::Focus(error.clone())),
    }
}

pub fn secure_notice(snapshot: &Snapshot, entry: &Entry) -> bool {
    snapshot.secure_desktop() && entry.kind == EntryKind::Display
}

pub fn window_id(id: EntryId) -> HostWindowId {
    HostWindowId(id.0)
}

pub fn entry_id(id: HostWindowId) -> EntryId {
    EntryId(id.0)
}

pub fn scale_at(snapshot: &Snapshot, rect: Rect) -> f32 {
    let centre = rect.centre();
    snapshot
        .monitors
        .iter()
        .find(|monitor| monitor.rect.contains(centre.x, centre.y))
        .or_else(|| snapshot.monitors.iter().find(|monitor| monitor.primary))
        .map_or(1.0, |monitor| monitor.scale)
        .max(0.1)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgramNames {
    by_id: HashMap<String, String>,
    by_app: HashMap<String, String>,
}

fn app_key(app: &str) -> String {
    app.trim().replace('/', "\\").to_lowercase()
}

impl ProgramNames {
    pub fn new(programs: &[Program]) -> Self {
        let mut names = Self::default();
        for program in programs {
            names.by_id.insert(program.id.clone(), program.name.clone());
            let shortcut = &program.shortcut;
            let target = shortcut
                .target
                .as_ref()
                .map(|target| target.to_string_lossy().into_owned());
            for app in shortcut.app_user_model_id.iter().chain(target.iter()) {
                names
                    .by_app
                    .entry(app_key(app))
                    .or_insert_with(|| program.name.clone());
            }
        }
        names
    }

    pub fn name(&self, entry: &Entry) -> String {
        let launched = entry
            .origin
            .program
            .as_ref()
            .and_then(|program| self.by_id.get(program));
        let matched = || {
            [entry.app_id.as_str()]
                .into_iter()
                .chain(entry.origin.command.first().map(String::as_str))
                .filter(|app| !app.trim().is_empty())
                .find_map(|app| self.by_app.get(&app_key(app)))
        };
        launched.or_else(matched).cloned().unwrap_or_default()
    }
}

pub fn host_window(
    snapshot: &Snapshot,
    entry: &Entry,
    shaping: &Shaping,
    notes: &Notes,
    names: &ProgramNames,
) -> HostWindow {
    let scale = scale_at(snapshot, entry.frame);
    let (width, height) = shaping.own_size(entry);
    let fullscreen = shaping.fullscreen_area(snapshot, entry);
    HostWindow {
        id: window_id(entry.id),
        title: entry.title.clone(),
        app_id: entry.app_id.clone(),
        name: match entry.kind {
            EntryKind::Window => names.name(entry),
            EntryKind::Display => String::new(),
        },
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
        note: notes.note(entry),
    }
}

pub fn listed(
    snapshot: &Snapshot,
    hidden: &[HostWindowId],
    shaping: &Shaping,
    notes: &Notes,
    names: &ProgramNames,
) -> Vec<HostWindow> {
    snapshot
        .entries
        .iter()
        .filter(|entry| !hidden.contains(&window_id(entry.id)))
        .map(|entry| {
            let mut window = host_window(snapshot, entry, shaping, notes, names);
            window.parent = window.parent.filter(|parent| !hidden.contains(parent));
            window
        })
        .collect()
}

pub fn power_availability(
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

pub fn power_action(action: block_plugin_api::PowerAction) -> be_win32::PowerAction {
    match action {
        block_plugin_api::PowerAction::Lock => be_win32::PowerAction::Lock,
        block_plugin_api::PowerAction::Suspend => be_win32::PowerAction::Suspend,
        block_plugin_api::PowerAction::Restart => be_win32::PowerAction::Restart,
        block_plugin_api::PowerAction::PowerOff => be_win32::PowerAction::PowerOff,
        block_plugin_api::PowerAction::LogOut => be_win32::PowerAction::LogOut,
    }
}

pub fn host_program(program: &Program, icon: Option<HostImage>) -> HostProgram {
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

pub fn frame_point(frame: Rect, panel: Vec2, local: block_plugin_api::Pos2) -> Point {
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

pub fn popup_rect(frame: Rect, popup: Rect, panel: Vec2) -> block_plugin_api::Rect {
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

pub fn popup_point(frame: Rect, popup: Rect, shown: Vec2, local: block_plugin_api::Pos2) -> Point {
    let at = frame_point(popup, shown, local);
    Point::new(popup.left - frame.left + at.x, popup.top - frame.top + at.y)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recovery {
    Restart,
    Notice(String),
    Stop,
}

impl Recovery {
    pub fn notice(self) -> Option<String> {
        match self {
            Self::Notice(notice) => Some(notice),
            Self::Restart | Self::Stop => None,
        }
    }
}

pub fn recovery(failed: Option<&str>, restarted: bool, title: &str) -> Recovery {
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
