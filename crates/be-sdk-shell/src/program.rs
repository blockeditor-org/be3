use std::collections::{BTreeMap, HashMap, HashSet};

use be_sdk_beui::beui::dock::{
    DockDialog, DockEntry, DockNode, DockPane, DockTab, DockWindow, TabId, use_dock_tab,
};
use be_sdk_beui::beui::dock_host::styled::WINDOW_CHROME;
use be_sdk_beui::beui::dock_host::unstyled::DockSnap;
use be_sdk_beui::beui::icons::{
    ICON_APP_WINDOW, ICON_EXTERNAL_LINK, ICON_EYE_OFF, ICON_FULLSCREEN, ICON_MONITOR, ICON_X,
};
use be_sdk_beui::beui::reactive::*;
use be_sdk_beui::beui::styled::*;
use be_sdk_beui::beui::{Image, NodeId, Pos2, Rect, Vec2, pos2, vec2};
use be_sdk_beui::desktop;
use be_sdk_beui::{
    ChildId, ChildMode, ChildState, HostProgram, HostWindow, HostWindowId, MenuAsk, Picture,
    ProgramAction, Subregion, SubregionContent, WindowKind,
};
use beui::{Align, Direction, Justify};
use serde::{Deserialize, Serialize};

use super::workspace::Workspace;

const DIALOG_TABS: u64 = 1 << 41;
const WINDOW_ORIGIN: f32 = 80.0;
const WINDOW_CASCADE: f32 = 32.0;
const WINDOW_CASCADES: u64 = 8;
const PLACEHOLDER_SIZE: Vec2 = Vec2::new(800.0, 500.0);
const ICON_POINTS: f32 = 64.0;
const PICTURE_SIZE: Size = Size::Xl3;
const BODY_SPACING: Space = Space::Lg;
const BUTTON_SPACING: Space = Space::Md;
const BODY_PADDING: Space = Space::Xl3;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProgramRecord {
    pub window: Option<HostWindowId>,
    pub program_id: String,
    pub program: Option<String>,
    pub command: Vec<String>,
    pub title: String,
    #[serde(default)]
    pub display: bool,
}

impl ProgramRecord {
    fn of(window: &HostWindow) -> Self {
        let mut record = Self::default();
        record.follow(window);
        record
    }

    fn follow(&mut self, window: &HostWindow) {
        self.window = Some(window.id);
        self.program_id.clone_from(&window.program_id);
        self.program.clone_from(&window.origin.program);
        self.command.clone_from(&window.origin.command);
        self.display = window.kind == WindowKind::Display;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Program {
    pub(crate) window: Option<HostWindowId>,
    pub(crate) record: ProgramRecord,
    pub(crate) sized: bool,
}

impl Program {
    pub(crate) fn restored(record: ProgramRecord) -> Self {
        Self {
            window: None,
            record,
            sized: true,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Heard {
    pub(crate) child: Option<ChildId>,
    pub(crate) title: Option<String>,
    pub(crate) icon: Option<Picture>,
    pub(crate) group: Option<String>,
    pub(crate) reported: bool,
    pub(crate) raised: u64,
    pub(crate) rung: u64,
    pub(crate) minimized: u64,
    pub(crate) fullscreen: bool,
    pub(crate) notice: Option<String>,
    pub(crate) menu: Option<MenuAsk>,
    pub(crate) intrinsic: Option<Vec2>,
    pub(crate) closing: bool,
}

pub(crate) type Programs = BTreeMap<TabId, Program>;

pub fn dialog_tab(window: HostWindowId) -> TabId {
    TabId::new(DIALOG_TABS + window.0)
}

pub(crate) fn listed_windows(entries: &[HostWindow]) -> Vec<HostWindow> {
    let listed: HashSet<HostWindowId> = entries.iter().map(|entry| entry.id).collect();
    entries
        .iter()
        .map(|window| HostWindow {
            parent: window.parent.filter(|parent| listed.contains(parent)),
            ..window.clone()
        })
        .collect()
}

/// Where a new program window or restored placeholder opens: a cascade step from its tab key,
/// with fewer steps when the area leaves little room. A new window opens at the size its program
/// picked (`HostWindow::size`), so the program is never told another size first; a placeholder, a
/// window filling one, and a window whose program picked no size open at the placeholder's size.
/// A new window whose program asked to be maximized opens snapped to fill its area.
/// There is no size negotiation after that: the dock sizes the window and the host fits the
/// program to it.
pub(crate) fn cascaded(key: u64, size: Vec2, area: Rect) -> Rect {
    if !area.is_positive() {
        let step = (key % WINDOW_CASCADES) as f32 * WINDOW_CASCADE;
        return Rect::from_min_size(pos2(WINDOW_ORIGIN + step, WINDOW_ORIGIN + step), size);
    }
    let size = size.min(area.size());
    let free = area.size() - size;
    let room = (free.x.min(free.y) - WINDOW_ORIGIN) / WINDOW_CASCADE;
    let cascades = match room >= 0.0 {
        true => (room as u64 + 1).min(WINDOW_CASCADES),
        false => 1,
    };
    let step = WINDOW_ORIGIN + (key % cascades) as f32 * WINDOW_CASCADE;
    Rect::from_min_size(area.min + vec2(step.min(free.x), step.min(free.y)), size)
}

pub fn picture_image(picture: &Picture) -> Option<Image> {
    let whole = picture.rgba.len() == picture.width as usize * picture.height as usize * 4;
    (picture.width > 0 && picture.height > 0 && whole)
        .then(|| Image::from_rgba(picture.width, picture.height, picture.rgba.clone()))
}

/// Binds each parentless listed window to a tab: the tab whose key is its `origin.launch`, else a
/// waiting tab that last held this window id for the same program (a shell restarted over the
/// same host), else the oldest waiting tab of its program id, else a new tab. While `ending`, a
/// window that leaves the list and can be started again leaves its tab as a placeholder.
pub(crate) fn place(
    programs: &mut Programs,
    windows: &[HostWindow],
    ending: bool,
    next_tab: &mut u64,
    relaunchable: impl Fn(&ProgramRecord) -> bool,
) -> Vec<TabId> {
    let mut top: Vec<&HostWindow> = windows
        .iter()
        .filter(|window| window.parent.is_none())
        .collect();
    top.sort_by_key(|window| window.id);
    let live: HashMap<HostWindowId, &HostWindow> =
        top.iter().map(|window| (window.id, *window)).collect();
    let mut gone = Vec::new();
    programs.retain(|tab, program| {
        let Some(window) = program.window else {
            return true;
        };
        match live.get(&window) {
            Some(listed) => {
                program.record.follow(listed);
                true
            }
            None if ending && relaunchable(&program.record) => {
                program.window = None;
                true
            }
            None => {
                gone.push(*tab);
                false
            }
        }
    });
    let bound: HashSet<HostWindowId> = programs
        .values()
        .filter_map(|program| program.window)
        .collect();
    for window in top {
        if bound.contains(&window.id) {
            continue;
        }
        let waiting = |program: &Program| program.window.is_none();
        let launched = window
            .origin
            .launch
            .map(TabId::new)
            .filter(|tab| programs.get(tab).is_some_and(waiting));
        let remembered = || {
            programs
                .iter()
                .find(|(_, program)| {
                    waiting(program)
                        && program.record.window == Some(window.id)
                        && program.record.program_id == window.program_id
                })
                .map(|(tab, _)| *tab)
        };
        let same_program = || {
            programs
                .iter()
                .find(|(_, program)| {
                    waiting(program)
                        && !window.program_id.is_empty()
                        && program.record.program_id == window.program_id
                })
                .map(|(tab, _)| *tab)
        };
        match launched.or_else(remembered).or_else(same_program) {
            Some(tab) => {
                if let Some(program) = programs.get_mut(&tab) {
                    program.window = Some(window.id);
                    program.record.follow(window);
                }
            }
            None => {
                let tab = TabId::new(*next_tab);
                *next_tab += 1;
                programs.insert(
                    tab,
                    Program {
                        window: Some(window.id),
                        record: ProgramRecord::of(window),
                        sized: false,
                    },
                );
            }
        }
    }
    gone
}

pub(crate) fn program_for<'a>(
    record: &ProgramRecord,
    programs: &'a [HostProgram],
) -> Option<&'a HostProgram> {
    if let Some(program) = &record.program {
        return programs.iter().find(|listed| listed.id == *program);
    }
    let program_id = record.program_id.as_str();
    if program_id.is_empty() {
        return None;
    }
    let stem = |program: &HostProgram| program.id.trim_end_matches(".desktop").to_owned();
    programs
        .iter()
        .find(|program| {
            program
                .wm_class
                .as_deref()
                .is_some_and(|class| class.eq_ignore_ascii_case(program_id))
        })
        .or_else(|| {
            programs
                .iter()
                .find(|program| stem(program).eq_ignore_ascii_case(program_id))
        })
        .or_else(|| {
            programs.iter().find(|program| {
                stem(program)
                    .rsplit('.')
                    .next()
                    .is_some_and(|last| last.eq_ignore_ascii_case(program_id))
            })
        })
}

pub(crate) fn launch_action(
    tab: TabId,
    record: &ProgramRecord,
    programs: &[HostProgram],
) -> Option<ProgramAction> {
    let launch = Some(tab.value());
    let program = record
        .program
        .clone()
        .or_else(|| program_for(record, programs).map(|program| program.id.clone()));
    match program {
        Some(program) => Some(ProgramAction::Launch { program, launch }),
        None if !record.command.is_empty() => Some(ProgramAction::Exec {
            command: record.command.clone(),
            launch,
        }),
        None => None,
    }
}

const NAME_END: &str = " \u{2014} ";

pub(crate) fn program_name(record: &ProgramRecord, program: Option<&HostProgram>) -> String {
    if let Some(program) = program {
        return program.name.clone();
    }
    let named = record
        .title
        .split(NAME_END)
        .next()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned);
    let command = || {
        record.command.first().map(|command| {
            command
                .rsplit('/')
                .next()
                .unwrap_or(command.as_str())
                .to_owned()
        })
    };
    let program_id = || {
        let id = record.program_id.trim();
        let file = id.rsplit(['/', '\\']).next().unwrap_or(id);
        let file = match file.len().checked_sub(4).and_then(|cut| file.get(cut..)) {
            Some(extension) if extension.eq_ignore_ascii_case(".exe") => &file[..file.len() - 4],
            _ => file,
        };
        Some(file.rsplit('.').next().unwrap_or(file).to_owned())
    };
    named
        .filter(|name| !name.is_empty())
        .or_else(|| program_id().filter(|name| !name.is_empty()))
        .or_else(command)
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Program".to_owned())
}

pub(crate) fn window_title(record: &ProgramRecord) -> String {
    match record.title.split_once(NAME_END) {
        Some((_, title)) => title.trim().to_owned(),
        None => String::new(),
    }
}

impl Workspace {
    pub(crate) fn place_windows(&self) {
        if !self.ready.get() {
            return;
        }
        let ending = self.ending.get();
        let windows = self.windows.get();
        untrack(|| {
            let mut programs = self.programs.get_untracked();
            let before: HashMap<TabId, Option<HostWindowId>> = programs
                .iter()
                .map(|(tab, program)| (*tab, program.window))
                .collect();
            let mut next = self.next_tab();
            let catalog = self.catalog.get_untracked();
            let gone = place(&mut programs, &windows, ending, &mut next, |record| {
                record.program.is_some()
                    || !record.command.is_empty()
                    || program_for(record, &catalog).is_some()
            });
            self.set_next_tab(next);
            let dialogs: HashSet<TabId> = windows
                .iter()
                .filter(|window| window.parent.is_some())
                .map(|window| dialog_tab(window.id))
                .collect();
            let rebound: HashSet<TabId> = programs
                .iter()
                .filter(|(tab, program)| {
                    program.window.is_some()
                        && before.get(tab).is_some_and(|held| *held != program.window)
                })
                .map(|(tab, _)| *tab)
                .collect();
            let kept = |tab: &TabId| {
                (programs.contains_key(tab) && !rebound.contains(tab)) || dialogs.contains(tab)
            };
            let stale = self
                .heard
                .with_untracked(|heard| heard.keys().any(|tab| !kept(tab)));
            if stale || !gone.is_empty() || !rebound.is_empty() {
                self.heard.update(|heard| heard.retain(|tab, _| kept(tab)));
            }
            if programs != self.programs.get_untracked() {
                self.programs.set(programs);
            }
        });
    }

    pub(crate) fn program(&self, tab: TabId) -> Option<Program> {
        self.programs.with(|programs| programs.get(&tab).cloned())
    }

    pub fn program_tabs(&self) -> Memo<Vec<TabId>> {
        let programs = self.programs;
        create_memo(move || programs.with(|programs| programs.keys().copied().collect()))
    }

    pub fn program_window(&self, tab: TabId) -> Option<HostWindowId> {
        self.programs
            .with(|programs| programs.get(&tab).and_then(|program| program.window))
    }

    pub fn is_display(&self, tab: TabId) -> bool {
        self.programs.with(|programs| {
            programs
                .get(&tab)
                .is_some_and(|program| program.record.display)
        })
    }

    pub fn running_names(&self) -> Vec<String> {
        self.programs.with(|programs| {
            self.catalog.with(|catalog| {
                let mut names: Vec<String> = Vec::new();
                for program in programs.values().filter(|program| program.window.is_some()) {
                    let name = program_name(&program.record, program_for(&program.record, catalog));
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
                names
            })
        })
    }

    pub fn placeholders(&self) -> Memo<Vec<TabId>> {
        let programs = self.programs;
        create_memo(move || {
            programs.with(|programs| {
                programs
                    .iter()
                    .filter(|(_, program)| program.window.is_none())
                    .map(|(tab, _)| *tab)
                    .collect()
            })
        })
    }

    pub fn launchable(&self) -> Memo<bool> {
        self.launchable
    }

    pub fn window_tab_of(&self, window: HostWindowId) -> TabId {
        self.programs
            .with(|programs| {
                programs
                    .iter()
                    .find(|(_, program)| program.window == Some(window))
                    .map(|(tab, _)| *tab)
            })
            .unwrap_or_else(|| dialog_tab(window))
    }

    pub(crate) fn heard_of(&self, tab: TabId) -> Memo<Heard> {
        let heard = self.heard;
        create_memo(move || heard.with(|heard| heard.get(&tab).cloned().unwrap_or_default()))
    }

    pub fn tab_picture(&self, tab: TabId) -> Memo<Option<Image>> {
        let heard = self.heard;
        let icon = create_memo(move || heard.with(|heard| heard.get(&tab)?.icon.clone()));
        let workspace = *self;
        create_memo(move || {
            icon.with(|icon| icon.as_ref().and_then(picture_image))
                .or_else(|| workspace.placeholder_picture(tab))
        })
    }

    pub fn tab_rung(&self, tab: TabId) -> Memo<u64> {
        let heard = self.heard;
        create_memo(move || heard.with(|heard| heard.get(&tab).map_or(0, |heard| heard.rung)))
    }

    fn placeholder_picture(&self, tab: TabId) -> Option<Image> {
        let program = self
            .program(tab)
            .filter(|program| program.window.is_none())?;
        self.catalog.with(|catalog| {
            let icon = program_for(&program.record, catalog)?.icon.as_ref()?;
            icon.is_whole()
                .then(|| Image::from_rgba(icon.width, icon.height, icon.rgba.clone()))
        })
    }

    pub(crate) fn program_title(&self, tab: TabId) -> Option<String> {
        let program = self.program(tab)?;
        let heard = self
            .heard
            .with(|heard| heard.get(&tab).and_then(|heard| heard.title.clone()));
        Some(match heard.or_else(|| Some(program.record.title.clone())) {
            Some(title) if !title.is_empty() => title,
            _ => self.catalog.with(|catalog| {
                program_name(&program.record, program_for(&program.record, catalog))
            }),
        })
    }

    pub(crate) fn hear(&self, tab: TabId, state: &ChildState) {
        let held = self
            .heard
            .with_untracked(|heard| heard.get(&tab).cloned().unwrap_or_default());
        let mut next = held.clone();
        next.child = state.child;
        if state.child.is_some() && state.available {
            next.reported = true;
            next.title.clone_from(&state.title);
            next.icon.clone_from(&state.icon);
            next.group.clone_from(&state.group);
            next.raised = state.raised;
            next.rung = state.rung;
            next.minimized = state.minimized;
            next.fullscreen = state.fullscreen;
            next.notice.clone_from(&state.notice);
            next.menu = state.menu_asked;
            next.intrinsic = state.intrinsic_size;
        }
        let close = next.closing && next.child.is_some();
        if close {
            next.closing = false;
        }
        if next != held {
            self.heard.update(|heard| {
                heard.insert(tab, next.clone());
            });
        }
        if let Some(title) = next.title.clone() {
            let changed = self.programs.with_untracked(|programs| {
                programs
                    .get(&tab)
                    .is_some_and(|program| program.record.title != title)
            });
            if changed {
                self.programs.update(|programs| {
                    if let Some(program) = programs.get_mut(&tab) {
                        program.record.title = title;
                    }
                });
            }
        }
        let child = next.child;
        if close && let Some(child) = child {
            self.host().close_child(child);
            return;
        }
        if next.fullscreen != held.fullscreen && !self.is_dialog(tab) {
            let layout = self.layout();
            let shown = layout.fullscreen() == Some(tab);
            match (shown, next.fullscreen) {
                (false, true) => layout.enter_fullscreen(&tab, self.screen_of(tab)),
                (true, false) => layout.leave_fullscreen(),
                _ => {}
            }
        }
    }

    fn screen_of(&self, tab: TabId) -> Option<Rect> {
        let screens = self.screens.get_untracked();
        let first = screens.first()?.min;
        let screens: Vec<Rect> = screens
            .iter()
            .map(|screen| screen.translate(Pos2::ZERO - first))
            .collect();
        let held = self.on_dock_host(tab, |host, tab| host.window_rect(tab))??;
        let at = be_sdk_beui::beui::dock_host::unstyled::holding_area(held, &screens)?;
        screens.get(at).copied()
    }

    fn is_dialog(&self, tab: TabId) -> bool {
        tab.value() >= DIALOG_TABS && self.program(tab).is_none()
    }

    fn root_tab(&self, tab: TabId) -> TabId {
        let mut tab = tab;
        let windows = self.windows.get_untracked();
        for _ in 0..windows.len() {
            if !self.is_dialog(tab) {
                break;
            }
            let window = HostWindowId(tab.value() - DIALOG_TABS);
            let Some(parent) = windows
                .iter()
                .find(|listed| listed.id == window)
                .and_then(|listed| listed.parent)
            else {
                break;
            };
            tab = untrack(|| self.window_tab_of(parent));
        }
        tab
    }

    pub(crate) fn close_program(&self, tab: TabId) -> bool {
        let Some(program) = self.program(tab) else {
            if self.is_dialog(tab) {
                self.close_window(tab);
                return true;
            }
            return false;
        };
        match program.window {
            Some(_) => self.close_window(tab),
            None => self.dismiss(tab),
        }
        true
    }

    fn close_window(&self, tab: TabId) {
        let child = self
            .heard
            .with_untracked(|heard| heard.get(&tab).and_then(|heard| heard.child));
        match child {
            Some(child) => self.host().close_child(child),
            None => {
                self.heard.update(|heard| {
                    heard.entry(tab).or_default().closing = true;
                });
                self.layout().show(&self.root_tab(tab));
            }
        }
    }

    pub fn dismiss(&self, tab: TabId) {
        if self.program_window(tab).is_some() || self.program(tab).is_none() {
            return;
        }
        self.programs.update(|programs| {
            programs.remove(&tab);
        });
        self.heard.update(|heard| {
            heard.remove(&tab);
        });
    }

    /// Set from the host's `PowerAvailability::ending`. The programs are asked to close only after
    /// that is published, so every window that closes during the ending is kept as a placeholder.
    pub fn keep_through_ending(&self, ending: bool) {
        if self.ending.get_untracked() != ending {
            self.ending.set(ending);
        }
    }

    pub fn placeholder_openable(&self, tab: TabId) -> Memo<bool> {
        let workspace = *self;
        create_memo(move || {
            workspace.launchable.get()
                && workspace.program(tab).is_some_and(|program| {
                    workspace
                        .catalog
                        .with(|catalog| launch_action(tab, &program.record, catalog).is_some())
                })
        })
    }

    pub fn open_placeholder(&self, tab: TabId) {
        let Some(program) = self.program(tab).filter(|program| program.window.is_none()) else {
            return;
        };
        let action = self
            .catalog
            .with_untracked(|catalog| launch_action(tab, &program.record, catalog));
        if let Some(action) = action {
            self.editor()
                .act(move || desktop::programs::request(action).detach());
        }
    }

    pub fn open_placeholders(&self) {
        let waiting: Vec<TabId> = self.programs.with_untracked(|programs| {
            programs
                .iter()
                .filter(|(_, program)| program.window.is_none())
                .map(|(tab, _)| *tab)
                .collect()
        });
        for tab in waiting {
            self.open_placeholder(tab);
        }
    }

    fn unshown(&self) -> Memo<Vec<(HostWindowId, TabId)>> {
        let workspace = *self;
        create_memo(move || {
            if !workspace.ready.get() {
                return Vec::new();
            }
            let windows = workspace.windows.get();
            windows
                .iter()
                .map(|window| (window.id, workspace.window_tab_of(window.id)))
                .filter(|(_, tab)| {
                    !workspace
                        .on_dock_host(workspace.root_tab(*tab), |host, tab| host.is_shown(tab))
                        .unwrap_or(false)
                })
                .collect()
        })
    }

    fn forget_child(&self, tab: TabId) {
        if !self.heard.is_alive() {
            return;
        }
        let held = self
            .heard
            .with_untracked(|heard| heard.get(&tab).is_some_and(|heard| heard.child.is_some()));
        if held {
            self.heard.update(|heard| {
                if let Some(heard) = heard.get_mut(&tab) {
                    heard.child = None;
                }
            });
        }
    }

    pub(crate) fn list_programs_for_placeholders(&self) {
        let placeholders = self.placeholders();
        let editor = self.editor();
        let scale = pixels_per_point();
        let listed = StoredCell::new(false);
        create_effect(move || {
            if placeholders.with(Vec::is_empty) || listed.get() {
                return;
            }
            listed.set(true);
            let icon_size = (ICON_POINTS * scale.get_untracked()).ceil() as u32;
            untrack(|| {
                editor.act(move || {
                    desktop::programs::request(ProgramAction::List { icon_size }).detach()
                })
            });
        });
    }

    /// The listing of a new tab's window, whose size it opens at; none for a tab that was
    /// restored or already sized.
    pub(crate) fn opening_window(&self, tab: TabId) -> Option<HostWindow> {
        if !self.needs_size(tab) || self.is_display(tab) {
            return None;
        }
        let window = self.program_window(tab)?;
        self.windows
            .with(|windows| windows.iter().find(|listed| listed.id == window).cloned())
    }

    pub(crate) fn needs_size(&self, tab: TabId) -> bool {
        self.programs
            .with_untracked(|programs| programs.get(&tab).is_some_and(|program| !program.sized))
    }

    pub(crate) fn sized(&self, tab: TabId) {
        if self.needs_size(tab) {
            self.programs.update(|programs| {
                if let Some(program) = programs.get_mut(&tab) {
                    program.sized = true;
                }
            });
        }
    }
}

#[component]
pub fn ProgramTab(
    workspace: Workspace,
    tab: TabId,
    #[prop(default = false)] floating: bool,
) -> DockEntry<TabId> {
    let title = workspace.tab_title(tab);
    let picture = workspace.tab_picture(tab);
    let glyph = match untrack(|| workspace.is_display(tab)) {
        true => ICON_MONITOR,
        false => ICON_APP_WINDOW,
    };
    let heard = workspace.heard_of(tab);
    let group = create_memo(move || {
        let group = heard.with(|heard| heard.group.clone()).or_else(|| {
            workspace
                .program(tab)
                .map(|program| program.record.program_id)
                .filter(|id| !id.is_empty())
        });
        group.map(|group| format!("program:{group}"))
    });
    let window = create_memo(move || workspace.program_window(tab));
    view! {
        <DockTab
            id=tab
            title
            icon=glyph
            picture
            group
            on_close={move || workspace.close(tab)}
            back_closes=false
        >
            <Layers>
                <Dynamic value={window}>|window: Option<HostWindowId>| {
                    match window {
                        Some(window) => view! {
                            <WindowBody workspace tab window floating />
                        },
                        None => view! {
                            <Placeholder workspace tab />
                        },
                    }
                }</Dynamic>
            </Layers>
        </DockTab>
    }
}

#[component]
pub fn ProgramWindow(workspace: Workspace, tab: TabId) -> DockNode<TabId> {
    let reach = Vec2::splat(be_sdk_beui::beui::dock_host::unstyled::HANDLE_REACH);
    let host = workspace.dock_host();
    let area = untrack(|| host.area()).size();
    let area = Rect::from_min_max(Pos2::ZERO + reach, Pos2::ZERO + area - reach);
    let opening = untrack(|| workspace.opening_window(tab));
    let picked = opening.as_ref().and_then(|listed| listed.size);
    let rect = cascaded(
        tab.value(),
        picked.unwrap_or(PLACEHOLDER_SIZE) + WINDOW_CHROME,
        area,
    );
    if opening.is_some_and(|listed| listed.maximized) {
        let snapped = StoredCell::new(false);
        create_effect(move || {
            let placed = workspace.on_dock_host(tab, |host, tab| host.window_rect(tab));
            if placed.flatten().is_some() && !snapped.replace(true) {
                untrack(|| {
                    workspace.on_dock_host(tab, |host, tab| {
                        host.snap_in_area(tab, DockSnap::Fill);
                    });
                });
            }
        });
    }
    if !untrack(|| workspace.is_display(tab)) {
        create_effect(move || untrack(|| workspace.sized(tab)));
    } else if workspace.needs_size(tab) {
        let heard = workspace.heard_of(tab);
        let named = create_memo(move || heard.with(|heard| heard.title.is_some()));
        create_effect(move || {
            let placed = workspace.on_dock_host(tab, |host, tab| host.window_rect(tab));
            if named.get() && placed.flatten().is_some() {
                untrack(|| {
                    workspace.sized(tab);
                    workspace.hide(tab);
                });
            }
        });
    }
    let pane = format!("window.{}", tab.value());
    view! {
        <DockWindow id={pane.clone()} rect>
            <DockPane id={pane}>
                <ProgramTab workspace tab floating=true />
            </DockPane>
        </DockWindow>
    }
}

#[component]
pub fn WindowDialog(
    workspace: Workspace,
    window: HostWindowId,
    parent: HostWindowId,
) -> DockNode<TabId> {
    let tab = dialog_tab(window);
    let listed = workspace.windows;
    let modal = create_memo(move || {
        listed.with(|windows| {
            windows
                .iter()
                .any(|listed| listed.id == window && listed.modal)
        })
    });
    let heard = workspace.heard_of(tab);
    let title = create_memo(move || {
        heard
            .with(|heard| heard.title.clone())
            .unwrap_or_else(|| "Window".to_owned())
    });
    // The listing's size until the window's status is first heard, so a new dialog opens at the
    // size its program picked rather than at the placeholder's for a frame (and its program is
    // never configured to that first).
    let size = create_memo(move || {
        let picked = |size: Option<Vec2>| size.filter(|size| size.x > 1.0 && size.y > 1.0);
        picked(heard.with(|heard| heard.intrinsic))
            .or_else(|| {
                picked(listed.with(|windows| {
                    windows
                        .iter()
                        .find(|listed| listed.id == window)
                        .and_then(|listed| listed.size)
                }))
            })
            .unwrap_or(PLACEHOLDER_SIZE)
    });
    view! {
        <DockDialog
            id={tab}
            parent={create_memo(move || workspace.window_tab_of(parent))}
            title={title}
            size
            modal
            on_close={move || workspace.close(tab)}
        >
            <WindowBody workspace tab window floating=false />
        </DockDialog>
    }
}

#[component]
fn WindowBody(workspace: Workspace, tab: TabId, window: HostWindowId, floating: bool) -> NodeId {
    let editor = workspace.editor();
    let layout = workspace.layout();
    let heard = workspace.heard_of(tab);
    let control = use_dock_tab();
    let fullscreen = create_memo(move || control.is_some_and(|control| control.fullscreen()));
    if let Some(control) = control {
        let notice = create_memo(move || heard.with(|heard| heard.notice.clone()));
        control.show_notice(notice);
        if floating {
            control.offer_fullscreen();
        }
    }
    on_cleanup(move || workspace.forget_child(tab));
    let minimized = create_memo(move || heard.with(|heard| heard.minimized));
    let seen_minimize = StoredCell::new(minimized.get_untracked());
    create_effect(move || {
        let now = minimized.get();
        if now > seen_minimize.replace(now) {
            untrack(|| workspace.hide(workspace.root_tab(tab)));
        }
    });
    let raised = create_memo(move || heard.with(|heard| heard.raised));
    let seen_raise = StoredCell::new(raised.get_untracked());
    create_effect(move || {
        let now = raised.get();
        if now <= seen_raise.replace(now) {
            return;
        }
        // A raise asks for the window to be shown, not proof that its keyboard went there, so
        // `show` rather than `content_focused`: the window's focus reaches the dock through its
        // child view, whose `ChildBlock` reports it.
        untrack(|| match control {
            Some(control) => control.show(),
            None => {
                layout.show(&tab);
                if let Some(child) = heard.with(|heard| heard.child) {
                    workspace.host().focus_child(child);
                }
            }
        });
    });
    if let Some(control) = control {
        let called = create_memo(move || control.attention());
        create_effect(move || {
            if called.get().is_some()
                && let Some(child) = untrack(|| heard.with(|heard| heard.child))
            {
                untrack(|| workspace.host().focus_child(child));
            }
        });
    }
    let panel = NodeRef::new();
    let menu_at = create_signal(None::<Pos2>);
    let asked = create_memo(move || heard.with(|heard| heard.menu));
    let seen = StoredCell::new(asked.get_untracked().map(|menu| menu.asked));
    create_effect(move || {
        let Some(asked) = asked.get() else {
            return;
        };
        if seen.replace(Some(asked.asked)) == Some(asked.asked) {
            return;
        }
        let shown = untrack(|| {
            let node = panel.try_get()?;
            measure(node).get_untracked()
        });
        if let Some(shown) = shown {
            menu_at.set(Some(shown.rect.min + asked.at.to_vec2()));
        }
    });
    let fullscreen_label = create_memo(move || match fullscreen.get() {
        true => "Leave full screen".to_owned(),
        false => "Full screen".to_owned(),
    });
    view! {
        <Layers>
            <ContextMenu
                disabled=true
                open_at={menu_at}
                on_close={move || menu_at.set(None)}
                items={view! {
                    <Show condition={floating}>
                        <MenuItem
                            label="Hide"
                            glyph=ICON_EYE_OFF
                            row_test_id="workspace.window.menu.hide"
                            on_click={move || workspace.hide(tab)}
                        />
                        <MenuItem
                            label={fullscreen_label}
                            glyph=ICON_FULLSCREEN
                            row_test_id="workspace.window.menu.fullscreen"
                            on_click={move || {
                                workspace.on_dock_host(tab, |host, tab| {
                                    host.toggle_window_fullscreen(tab);
                                });
                            }}
                        />
                    </Show>
                    <MenuItem
                        label="Close"
                        glyph=ICON_X
                        row_test_id="workspace.window.menu.close"
                        on_click={move || workspace.close(tab)}
                    />
                }}
            >
                <Frame />
            </ContextMenu>
            <Subregion
                editor={editor}
                placed={Some(SubregionContent::Window(window))}
                mode=ChildMode::Live
                fullscreen={fullscreen}
                on_state={Callback::new(move |state: ChildState| workspace.hear(tab, &state))}
                @test_id={"workspace.window"}
                @node_ref=&panel
            />
        </Layers>
    }
}

#[component]
pub fn UnshownWindows(workspace: Workspace) -> NodeId {
    let unshown = workspace.unshown();
    view! {
        <Frame width={Size::Custom(0.0)} height={Size::Custom(0.0)}>
            <Layers>
                <ForEach keys={unshown}>|(window, tab): (HostWindowId, TabId)|
                    <UnshownWindow workspace window tab />
                </ForEach>
            </Layers>
        </Frame>
    }
}

#[component]
fn UnshownWindow(workspace: Workspace, window: HostWindowId, tab: TabId) -> NodeId {
    let editor = workspace.editor();
    let layout = workspace.layout();
    let heard = workspace.heard_of(tab);
    on_cleanup(move || workspace.forget_child(tab));
    let raised = create_memo(move || heard.with(|heard| heard.reported.then_some(heard.raised)));
    let seen = StoredCell::new(raised.get_untracked());
    create_effect(move || {
        let Some(now) = raised.get() else {
            return;
        };
        let before = seen.replace(Some(now));
        if before.is_none_or(|before| now <= before) {
            return;
        }
        untrack(|| {
            let root = workspace.root_tab(tab);
            layout.show(&root);
            if root != tab {
                layout.show(&tab);
            }
        });
    });
    view! {
        <Subregion
            editor={editor}
            placed={Some(SubregionContent::Window(window))}
            mode=ChildMode::Hidden
            punch=false
            on_state={Callback::new(move |state: ChildState| workspace.hear(tab, &state))}
            @test_id={format!("workspace.window.{}.unshown", window.0)}
        />
    }
}

#[component]
fn Placeholder(workspace: Workspace, tab: TabId) -> NodeId {
    let catalog = workspace.catalog;
    let record = create_memo(move || workspace.program(tab).map(|program| program.record));
    let name = create_memo(move || {
        record.with(|record| {
            record.as_ref().map_or_else(String::new, |record| {
                catalog.with(|catalog| program_name(record, program_for(record, catalog)))
            })
        })
    });
    let heading = create_memo(move || format!("{} is not running", name.get()));
    let title = create_memo(move || {
        record.with(|record| record.as_ref().map(window_title).unwrap_or_default())
    });
    let picture = create_memo(move || workspace.placeholder_picture(tab));
    let pictured = create_memo(move || picture.with(Option::is_some));
    let plain = create_memo(move || !pictured.get());
    let openable = workspace.placeholder_openable(tab);
    let unopenable = create_memo(move || !openable.get());
    view! {
        <Frame
            fill=ColorRole::Background
            padding=BODY_PADDING
            @test_id={format!("workspace.placeholder.{}", tab.value())}
        >
            <List spacing=BODY_SPACING align=Align::Center justify=Justify::Center>
                <Show condition={pictured}>
                    <Frame width=PICTURE_SIZE height=PICTURE_SIZE>
                        <Picture image={picture} />
                    </Frame>
                </Show>
                <Show condition={plain}>
                    <Text size=TextSize::Xl5 color=TextColor::Muted>
                        <Icon glyph=ICON_APP_WINDOW />
                    </Text>
                </Show>
                <Text style=TextStyle::HEADING>{heading}</Text>
                <Text style=TextStyle::CAPTION>{title}</Text>
                <List direction=Direction::Horizontal align=Align::Center spacing=BUTTON_SPACING>
                    <Button
                        label="Open"
                        glyph=ICON_EXTERNAL_LINK
                        variant=ButtonVariant::Primary
                        disabled={unopenable}
                        @test_id={format!("workspace.placeholder.{}.open", tab.value())}
                        on_click={move || workspace.open_placeholder(tab)}
                    />
                    <Button
                        label="Dismiss"
                        variant=ButtonVariant::Secondary
                        @test_id={format!("workspace.placeholder.{}.dismiss", tab.value())}
                        on_click={move || workspace.dismiss(tab)}
                    />
                </List>
            </List>
        </Frame>
    }
}

#[cfg(test)]
mod tests;
