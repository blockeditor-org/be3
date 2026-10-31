use std::collections::{HashMap, HashSet};

use block_editor_beui::{HostWindow, HostWindowId, WindowOrigin};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeptWindow {
    pub key: HostWindowId,
    pub window: Option<HostWindowId>,
    pub app_id: String,
    #[serde(default)]
    pub name: String,
    pub title: String,
    pub origin: WindowOrigin,
    pub fullscreen: bool,
}

impl KeptWindow {
    fn of(key: HostWindowId, window: &HostWindow) -> Self {
        Self {
            key,
            window: Some(window.id),
            app_id: window.app_id.clone(),
            name: window.name.clone(),
            title: window.title.clone(),
            origin: window.origin.clone(),
            fullscreen: window.fullscreen.is_some(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Kept {
    pub(crate) restored: bool,
    pub(crate) records: Vec<KeptWindow>,
    pub(crate) seen: HashSet<HostWindowId>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Desk {
    pub(crate) bound: HashMap<HostWindowId, HostWindowId>,
    pub(crate) claims: Vec<(HostWindowId, HostWindowId)>,
    pub(crate) placeholders: Vec<HostWindowId>,
    pub(crate) keys: Vec<HostWindowId>,
}

impl Desk {
    pub(crate) fn key(&self, window: HostWindowId) -> HostWindowId {
        self.bound.get(&window).copied().unwrap_or(window)
    }

    pub(crate) fn window(&self, key: HostWindowId) -> Option<HostWindowId> {
        self.bound
            .iter()
            .find(|(_, bound)| **bound == key)
            .map(|(window, _)| *window)
    }
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
        match record.window {
            Some(window) if open.contains(&window) => {
                desk.bound.insert(window, record.key);
            }
            Some(window) if kept.seen.contains(&window) => {}
            _ => free.push(record),
        }
    }
    for window in &live {
        if !kept.restored || kept.seen.contains(&window.id) || desk.bound.contains_key(&window.id) {
            continue;
        }
        let launched = free
            .iter()
            .position(|record| Some(record.key.0) == window.origin.launch);
        let same_program = || {
            free.iter()
                .position(|record| !record.app_id.is_empty() && record.app_id == window.app_id)
        };
        if let Some(index) = launched.or_else(same_program) {
            let record = free.remove(index);
            desk.bound.insert(window.id, record.key);
            desk.claims.push((window.id, record.key));
        }
    }
    for window in &live {
        desk.bound.entry(window.id).or_insert(window.id);
    }
    desk.placeholders = free.iter().map(|record| record.key).collect();
    desk.keys = live
        .iter()
        .map(|window| desk.key(window.id))
        .chain(desk.placeholders.iter().copied())
        .collect();
    desk.keys.sort();
    desk
}

pub(crate) fn settle(kept: &Kept, desk: &Desk, windows: &[HostWindow]) -> Kept {
    let live: HashSet<HostWindowId> = top(windows).iter().map(|window| window.id).collect();
    let mut next = kept.clone();
    for (window, key) in &desk.claims {
        if let Some(record) = next.records.iter_mut().find(|record| record.key == *key) {
            record.window = Some(*window);
        }
    }
    next.seen.extend(live.iter().copied());
    next.records.retain(|record| {
        record
            .window
            .is_none_or(|window| live.contains(&window) || !next.seen.contains(&window))
    });
    next
}

pub(crate) fn saved(kept: &Kept, desk: &Desk, windows: &[HostWindow]) -> Vec<KeptWindow> {
    let mut saved: Vec<KeptWindow> = top(windows)
        .into_iter()
        .map(|window| KeptWindow::of(desk.key(window.id), window))
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
