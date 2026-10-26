use std::collections::HashMap;

use be_win32::{Control, Entry, EntryId, EntryKind, Handle, Snapshot};
use block_plugin_api::{Rect, Vec2};

use super::win32_map::scale_at;

const SLACK: i32 = 2;
const ASKS_KEPT: usize = 8;

type Pixels = (i32, i32);

#[derive(Clone, Debug, Default)]
struct Sizing {
    own: Pixels,
    asks: Vec<Pixels>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Shaping {
    sizes: HashMap<EntryId, Sizing>,
    fullscreen: HashMap<EntryId, bool>,
    area: Option<Rect>,
}

fn near(a: Pixels, b: Pixels) -> bool {
    (a.0 - b.0).abs() <= SLACK && (a.1 - b.1).abs() <= SLACK
}

fn frame_size(entry: &Entry) -> Pixels {
    (entry.frame.width(), entry.frame.height())
}

pub(crate) fn covers_a_monitor(snapshot: &Snapshot, entry: &Entry) -> bool {
    entry.kind == EntryKind::Window
        && snapshot
            .monitors
            .iter()
            .any(|monitor| monitor.rect == entry.frame)
}

impl Shaping {
    pub(crate) fn observe(&mut self, snapshot: &Snapshot) {
        self.sizes
            .retain(|id, _| snapshot.entry(*id).is_some_and(|entry| entry.kind == EntryKind::Window));
        for entry in &snapshot.entries {
            if entry.kind != EntryKind::Window || entry.minimized {
                continue;
            }
            let size = frame_size(entry);
            let sizing = self.sizes.entry(entry.id).or_insert_with(|| Sizing {
                own: size,
                asks: Vec::new(),
            });
            if let Some(answered) = sizing.asks.iter().rposition(|ask| near(*ask, size)) {
                sizing.asks.drain(..answered);
            } else if !near(sizing.own, size) {
                sizing.own = size;
                sizing.asks.clear();
            }
        }
        self.fullscreen.retain(|id, seen| {
            let Some(entry) = snapshot.entry(*id) else {
                return false;
            };
            if entry.maximized {
                *seen = true;
            }
            !(*seen && !entry.maximized)
        });
    }

    pub(crate) fn own_size(&self, entry: &Entry) -> Pixels {
        self.sizes
            .get(&entry.id)
            .map_or_else(|| frame_size(entry), |sizing| sizing.own)
    }

    pub(crate) fn placed(&mut self, snapshot: &Snapshot, id: EntryId, size: Vec2) -> Option<Pixels> {
        let entry = snapshot.entry(id)?;
        if entry.kind != EntryKind::Window
            || entry.minimized
            || entry.maximized
            || entry.control != Control::Full
            || covers_a_monitor(snapshot, entry)
            || size.x < 1.0
            || size.y < 1.0
        {
            return None;
        }
        let scale = scale_at(snapshot, entry.frame);
        let wanted = (
            (size.x * scale).round() as i32,
            (size.y * scale).round() as i32,
        );
        let sizing = self.sizes.entry(id).or_insert_with(|| Sizing {
            own: frame_size(entry),
            asks: Vec::new(),
        });
        let heading = sizing.asks.last().copied().unwrap_or(frame_size(entry));
        if near(wanted, heading) {
            return None;
        }
        if sizing.asks.len() >= ASKS_KEPT {
            sizing.asks.remove(0);
        }
        sizing.asks.push(wanted);
        Some(wanted)
    }

    pub(crate) fn set_area(&mut self, area: Rect) -> bool {
        let changed = self.area != Some(area);
        self.area = Some(area);
        changed
    }

    pub(crate) fn fullscreen_area(&self, snapshot: &Snapshot, entry: &Entry) -> Option<Rect> {
        let asked = self.fullscreen.contains_key(&entry.id) && entry.maximized;
        let fullscreen =
            entry.kind == EntryKind::Window && (asked || covers_a_monitor(snapshot, entry));
        self.area.filter(|_| fullscreen)
    }

    pub(crate) fn enter_fullscreen(&mut self, snapshot: &Snapshot, id: EntryId) -> Option<Handle> {
        let entry = snapshot.entry(id)?;
        if entry.kind != EntryKind::Window || covers_a_monitor(snapshot, entry) {
            return None;
        }
        self.fullscreen.insert(id, entry.maximized);
        (!entry.maximized).then_some(entry.handle)
    }

    pub(crate) fn leave_fullscreen(&mut self, snapshot: &Snapshot, id: EntryId) -> Option<Handle> {
        self.fullscreen.remove(&id)?;
        let entry = snapshot.entry(id)?;
        entry.maximized.then_some(entry.handle)
    }
}

#[cfg(test)]
mod tests;
