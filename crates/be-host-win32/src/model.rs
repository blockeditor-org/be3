use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::filter::{Windows, alt_tab, on_screen};
use crate::integrity::control;
use crate::{
    Control, Desktop, Event, Handle, Image, InputDesktop, IntegrityLevel, Launch, Launches,
    Monitor, Origin, Process, RawWindow, Rect, monitors,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(pub u64);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EntryKind {
    #[default]
    Window,
    Display,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Popup {
    pub handle: Handle,
    pub frame: Rect,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Entry {
    pub id: EntryId,
    pub kind: EntryKind,
    pub handle: Handle,
    pub process_id: u32,
    pub title: String,
    pub app_id: String,
    pub parent: Option<EntryId>,
    pub modal: bool,
    pub frame: Rect,
    pub minimized: bool,
    pub maximized: bool,
    pub responding: bool,
    pub focused: bool,
    pub icon: Option<Arc<Image>>,
    pub raised: u64,
    pub shown: bool,
    pub control: Control,
    pub origin: Origin,
    pub popups: Vec<Popup>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub revision: u64,
    pub entries: Vec<Entry>,
    pub monitors: Vec<Monitor>,
    pub input_desktop: InputDesktop,
}

impl Snapshot {
    pub fn entry(&self, id: EntryId) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    pub fn secure_desktop(&self) -> bool {
        self.input_desktop.secure()
    }
}

#[derive(Clone, Debug)]
struct Known {
    id: EntryId,
    process_id: u32,
    origin: Option<Origin>,
}

#[derive(Default)]
pub struct Model {
    next_id: u64,
    known: HashMap<Handle, Known>,
    displays: HashMap<String, EntryId>,
    icons: HashMap<Handle, Option<Arc<Image>>>,
    app_ids: HashMap<Handle, Option<String>>,
    processes: HashMap<u32, Process>,
    shown: HashSet<EntryId>,
    raised: HashMap<EntryId, u64>,
    foreground: Option<EntryId>,
    launches: Launches,
    own: Option<(u32, Option<IntegrityLevel>)>,
    monitors_stale: bool,
    input_desktop_stale: bool,
    started: bool,
    snapshot: Arc<Snapshot>,
}

impl Model {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.clone()
    }

    pub fn show(&mut self, id: EntryId) {
        self.shown.insert(id);
    }

    pub fn hide(&mut self, id: EntryId) {
        self.shown.remove(&id);
    }

    pub fn launched(&mut self, launch: Launch) {
        let process_id = launch.process_id;
        self.launches.add(launch);
        let Some(process_id) = process_id else {
            return;
        };
        for known in self.known.values_mut() {
            if known.process_id == process_id
                && known
                    .origin
                    .as_ref()
                    .is_none_or(|origin| origin.program.is_none() && origin.launch.is_none())
            {
                known.origin = None;
            }
        }
    }

    pub fn update(&mut self, desktop: &dyn Desktop, events: &[Event]) -> bool {
        if !self.started {
            self.started = true;
            self.monitors_stale = true;
            self.input_desktop_stale = true;
        }
        for event in events {
            match *event {
                Event::Created(handle) | Event::Shown(handle) => {
                    self.icons.remove(&handle);
                    self.app_ids.remove(&handle);
                }
                Event::Destroyed(handle) => {
                    self.known.remove(&handle);
                    self.icons.remove(&handle);
                    self.app_ids.remove(&handle);
                }
                Event::DisplaysChanged => self.monitors_stale = true,
                Event::DesktopSwitched => self.input_desktop_stale = true,
                _ => {}
            }
        }
        let (own_process, own_integrity) = *self
            .own
            .get_or_insert_with(|| (desktop.own_process_id(), desktop.own_integrity()));
        let mut monitors = self.snapshot.monitors.clone();
        if std::mem::take(&mut self.monitors_stale) {
            monitors = desktop.monitors();
            monitors::sort(&mut monitors);
        }
        let mut input_desktop = self.snapshot.input_desktop.clone();
        if std::mem::take(&mut self.input_desktop_stale) {
            input_desktop = desktop.input_desktop();
        }

        let raw: Vec<RawWindow> = desktop
            .windows()
            .into_iter()
            .filter(|window| window.process_id != own_process)
            .collect();
        let index = Windows::new(&raw);
        let live: HashSet<Handle> = raw.iter().map(|window| window.handle).collect();
        let processes: HashSet<u32> = raw.iter().map(|window| window.process_id).collect();
        self.known.retain(|handle, _| live.contains(handle));
        self.icons.retain(|handle, _| live.contains(handle));
        self.app_ids.retain(|handle, _| live.contains(handle));
        self.processes
            .retain(|process, _| processes.contains(process));

        let selected: Vec<&RawWindow> = raw
            .iter()
            .filter(|window| alt_tab(window, &index))
            .collect();
        let mut ids: HashMap<Handle, EntryId> = HashMap::new();
        for window in &selected {
            let id = match self.known.get(&window.handle) {
                Some(known) if known.process_id == window.process_id => known.id,
                _ => {
                    self.next_id += 1;
                    let id = EntryId(self.next_id);
                    self.known.insert(
                        window.handle,
                        Known {
                            id,
                            process_id: window.process_id,
                            origin: None,
                        },
                    );
                    id
                }
            };
            ids.insert(window.handle, id);
        }
        let root = |window: &RawWindow| -> Option<EntryId> {
            ids.get(&window.handle).copied().or_else(|| {
                index
                    .owners(window)
                    .into_iter()
                    .find_map(|owner| ids.get(&owner.handle).copied())
            })
        };

        let foreground = desktop.foreground();
        let mut popups: HashMap<EntryId, Vec<Popup>> = HashMap::new();
        let mut popup_of: HashMap<Handle, EntryId> = HashMap::new();
        for window in raw.iter().rev() {
            if ids.contains_key(&window.handle) || !on_screen(window) || window.minimized {
                continue;
            }
            let target = root(window).or_else(|| {
                if !window.style.popup {
                    return None;
                }
                let same_thread: Vec<&&RawWindow> = selected
                    .iter()
                    .filter(|entry| entry.thread_id == window.thread_id)
                    .collect();
                let owner = same_thread
                    .iter()
                    .find(|entry| Some(entry.handle) == foreground)
                    .or(same_thread.first())?;
                ids.get(&owner.handle).copied()
            });
            if let Some(target) = target {
                popup_of.insert(window.handle, target);
                popups.entry(target).or_default().push(Popup {
                    handle: window.handle,
                    frame: window.frame,
                });
            }
        }
        let focused = foreground.and_then(|handle| {
            ids.get(&handle)
                .copied()
                .or_else(|| popup_of.get(&handle).copied())
                .or_else(|| index.get(handle).and_then(|window| root(window)))
        });
        if focused != self.foreground {
            if let Some(id) = focused {
                *self.raised.entry(id).or_default() += 1;
            }
            self.foreground = focused;
        }

        let mut entries = Vec::new();
        let devices: HashSet<&str> = monitors
            .iter()
            .map(|monitor| monitor.device.as_str())
            .collect();
        self.displays
            .retain(|device, _| devices.contains(device.as_str()));
        for monitor in &monitors {
            let id = match self.displays.get(&monitor.device) {
                Some(id) => *id,
                None => {
                    self.next_id += 1;
                    let id = EntryId(self.next_id);
                    self.displays.insert(monitor.device.clone(), id);
                    id
                }
            };
            entries.push(Entry {
                id,
                kind: EntryKind::Display,
                handle: monitor.handle,
                title: monitor.name.clone(),
                frame: monitor.rect,
                responding: true,
                shown: self.shown.contains(&id),
                ..Entry::default()
            });
        }

        let mut windows = Vec::new();
        for window in &selected {
            let id = ids[&window.handle];
            let process = self
                .processes
                .entry(window.process_id)
                .or_insert_with(|| desktop.process(window.process_id))
                .clone();
            let icon = self
                .icons
                .entry(window.handle)
                .or_insert_with(|| desktop.icon(window.handle))
                .clone();
            let app_id = self
                .app_ids
                .entry(window.handle)
                .or_insert_with(|| desktop.app_user_model_id(window.handle))
                .clone()
                .or_else(|| {
                    process
                        .exe
                        .as_ref()
                        .map(|exe| exe.to_string_lossy().into_owned())
                })
                .unwrap_or_default();
            let known = self
                .known
                .get_mut(&window.handle)
                .expect("every entry is known");
            let origin = known
                .origin
                .get_or_insert_with(|| {
                    self.launches
                        .origin(window.process_id, process.exe.as_deref())
                })
                .clone();
            let owner = index.visible_owner(window);
            let parent = owner
                .and_then(|owner| root(owner))
                .filter(|parent| *parent != id);
            windows.push(Entry {
                id,
                kind: EntryKind::Window,
                handle: window.handle,
                process_id: window.process_id,
                title: desktop.title(window.handle),
                app_id,
                parent,
                modal: parent.is_some() && owner.is_some_and(|owner| !owner.enabled),
                frame: window.frame,
                minimized: window.minimized,
                maximized: window.maximized,
                responding: window.responding,
                focused: focused == Some(id),
                icon,
                raised: self.raised.get(&id).copied().unwrap_or_default(),
                shown: self.shown.contains(&id),
                control: control(own_integrity, process.integrity),
                origin,
                popups: popups.remove(&id).unwrap_or_default(),
            });
        }
        windows.sort_by_key(|entry| entry.id);
        entries.extend(windows);
        self.launches.retain_processes(&processes);

        let present: HashSet<EntryId> = entries
            .iter()
            .map(|entry| entry.id)
            .chain(self.known.values().map(|known| known.id))
            .collect();
        self.shown.retain(|id| present.contains(id));
        self.raised.retain(|id, _| present.contains(id));

        let current = &self.snapshot;
        if current.entries == entries
            && current.monitors == monitors
            && current.input_desktop == input_desktop
        {
            return false;
        }
        self.snapshot = Arc::new(Snapshot {
            revision: current.revision + 1,
            entries,
            monitors,
            input_desktop,
        });
        true
    }
}
