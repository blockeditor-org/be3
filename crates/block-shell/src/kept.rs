use std::collections::{HashMap, HashSet};

use block_editor_beui::beui::unstyled::TabId;
use block_editor_beui::{HostWindow, HostWindowId, WindowOrigin};
use serde::{Deserialize, Serialize};

const PLACEHOLDER_TABS: u64 = 1 << 60;

pub fn placeholder_tab(key: HostWindowId) -> TabId {
    TabId::new(PLACEHOLDER_TABS + key.0)
}

pub(crate) fn tab_placeholder(tab: TabId) -> Option<HostWindowId> {
    tab.value().checked_sub(PLACEHOLDER_TABS).map(HostWindowId)
}

pub(crate) fn below_placeholders(tab: TabId) -> bool {
    tab.value() < PLACEHOLDER_TABS
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeptWindow {
    pub key: HostWindowId,
    pub app_id: String,
    pub title: String,
    pub origin: WindowOrigin,
    pub fullscreen: bool,
}

impl KeptWindow {
    fn of(window: &HostWindow) -> Self {
        Self {
            key: window.id,
            app_id: window.app_id.clone(),
            title: window.title.clone(),
            origin: window.origin.clone(),
            fullscreen: window.fullscreen.is_some(),
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum DeskEntry {
    Window(HostWindowId),
    Placeholder(HostWindowId),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Kept {
    pub(crate) restored: bool,
    pub(crate) records: Vec<KeptWindow>,
    pub(crate) filled: HashMap<HostWindowId, HostWindowId>,
    pub(crate) seen: HashSet<HostWindowId>,
}

impl Kept {
    fn window(&self, record: &KeptWindow) -> HostWindowId {
        self.filled.get(&record.key).copied().unwrap_or(record.key)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Desk {
    pub(crate) filling: HashMap<HostWindowId, HostWindowId>,
    pub(crate) claims: Vec<(HostWindowId, HostWindowId)>,
    pub(crate) placeholders: Vec<HostWindowId>,
    pub(crate) entries: Vec<DeskEntry>,
}

fn top(windows: &[HostWindow]) -> Vec<&HostWindow> {
    let mut top: Vec<&HostWindow> = windows
        .iter()
        .filter(|window| window.parent.is_none())
        .collect();
    top.sort_by_key(|window| window.id);
    top
}

pub(crate) fn sort_out(kept: &Kept, windows: &[HostWindow]) -> Desk {
    let live = top(windows);
    let open: HashSet<HostWindowId> = live.iter().map(|window| window.id).collect();
    let mut desk = Desk::default();
    let mut records: Vec<&KeptWindow> = kept.records.iter().collect();
    records.sort_by_key(|record| record.key);
    let mut free = Vec::new();
    for record in records {
        let window = kept.window(record);
        if open.contains(&window) {
            desk.filling.insert(window, record.key);
        } else if !kept.seen.contains(&window) {
            free.push(record);
        }
    }
    let claiming: Vec<&HostWindow> = live
        .iter()
        .copied()
        .filter(|window| {
            kept.restored
                && !kept.seen.contains(&window.id)
                && !desk.filling.contains_key(&window.id)
        })
        .collect();
    for window in claiming {
        let launched = free
            .iter()
            .position(|record| Some(record.key.0) == window.origin.launch);
        let same_program = || {
            free.iter()
                .position(|record| !record.app_id.is_empty() && record.app_id == window.app_id)
        };
        if let Some(index) = launched.or_else(same_program) {
            let record = free.remove(index);
            desk.filling.insert(window.id, record.key);
            desk.claims.push((window.id, record.key));
        }
    }
    desk.placeholders = free.iter().map(|record| record.key).collect();
    let mut entries: Vec<(HostWindowId, DeskEntry)> = live
        .iter()
        .map(|window| {
            let place = desk.filling.get(&window.id).copied().unwrap_or(window.id);
            (place, DeskEntry::Window(window.id))
        })
        .chain(
            desk.placeholders
                .iter()
                .map(|key| (*key, DeskEntry::Placeholder(*key))),
        )
        .collect();
    entries.sort_by_key(|(place, _)| *place);
    desk.entries = entries.into_iter().map(|(_, entry)| entry).collect();
    desk
}

pub(crate) fn settle(kept: &Kept, desk: &Desk, windows: &[HostWindow]) -> Kept {
    let live: HashSet<HostWindowId> = top(windows).iter().map(|window| window.id).collect();
    let mut next = kept.clone();
    for (window, key) in &desk.claims {
        next.filled.insert(*key, *window);
    }
    next.seen.extend(live.iter().copied());
    let closed: Vec<HostWindowId> = next
        .records
        .iter()
        .filter(|record| {
            let window = next.window(record);
            !live.contains(&window) && next.seen.contains(&window)
        })
        .map(|record| record.key)
        .collect();
    next.records.retain(|record| !closed.contains(&record.key));
    next.filled.retain(|key, _| !closed.contains(key));
    next
}

pub(crate) fn saved(kept: &Kept, desk: &Desk, windows: &[HostWindow]) -> Vec<KeptWindow> {
    let mut saved: Vec<KeptWindow> = top(windows)
        .into_iter()
        .map(KeptWindow::of)
        .chain(
            kept.records
                .iter()
                .filter(|record| desk.placeholders.contains(&record.key))
                .cloned(),
        )
        .collect();
    saved.sort_by_key(|record| record.key);
    saved
}
