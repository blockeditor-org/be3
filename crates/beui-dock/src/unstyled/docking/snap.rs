use beui_view::reactive::*;
use beui_view::types::{Pos2, Rect, pos2};

use super::{DockKey, DockingLayout, TabId, standing_in};

const SNAP_SLACK: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DockSnap {
    Left,
    Right,
    Fill,
}

impl DockSnap {
    pub fn rect(self, area: Rect) -> Rect {
        let middle = (area.min.x + area.width() / 2.0).round();
        match self {
            DockSnap::Left => Rect::from_min_max(area.min, pos2(middle, area.max.y)),
            DockSnap::Right => Rect::from_min_max(pos2(middle, area.min.y), area.max),
            DockSnap::Fill => area,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Snapped {
    snap: DockSnap,
    rect: Rect,
    restore: Rect,
}

fn same(left: Rect, right: Rect) -> bool {
    let near = |a: Pos2, b: Pos2| (a.x - b.x).abs() <= SNAP_SLACK && (a.y - b.y).abs() <= SNAP_SLACK;
    near(left.min, right.min) && near(left.max, right.max)
}

impl<K: DockKey> DockingLayout<K> {
    pub fn window_snap(&self, key: &K) -> Option<DockSnap> {
        let tab = self.tab_of(key)?;
        let rect = self.window_rect(key)?;
        self.snaps.with(|snaps| {
            snaps
                .get(&tab)
                .filter(|snapped| same(snapped.rect, rect))
                .map(|snapped| snapped.snap)
        })
    }

    pub fn snap_window(&self, key: &K, snap: DockSnap, area: Rect) {
        let Some(tab) = untrack(|| self.tab_of(key)) else {
            return;
        };
        let Some(rect) = untrack(|| self.window_rect(key)) else {
            return;
        };
        let restore = match untrack(|| self.window_snap(key)) {
            Some(_) => self.kept_snap(tab).map_or(rect, |snapped| snapped.restore),
            None => rect,
        };
        let snapped = snap.rect(area.shrink(crate::unstyled::HANDLE_REACH));
        self.place_window(tab, snapped);
        self.snaps.update(|snaps| {
            snaps.insert(
                tab,
                Snapped {
                    snap,
                    rect: snapped,
                    restore,
                },
            );
        });
    }

    pub fn unsnap_window(&self, key: &K) -> bool {
        let Some(tab) = untrack(|| self.tab_of(key)) else {
            return false;
        };
        let snapped = untrack(|| self.window_snap(key)).and_then(|_| self.kept_snap(tab));
        if self.kept_snap(tab).is_some() {
            self.snaps.update(|snaps| {
                snaps.remove(&tab);
            });
        }
        let Some(snapped) = snapped else {
            return false;
        };
        self.place_window(tab, snapped.restore);
        true
    }

    fn kept_snap(&self, tab: TabId) -> Option<Snapped> {
        self.snaps
            .with_untracked(|snaps| snaps.get(&tab).copied())
    }

    pub(super) fn place_window(&self, tab: TabId, rect: Rect) {
        let rect = rect.translate(-self.area.get_untracked().min.to_vec2());
        self.state.update(|state| {
            let Some(surface) = state
                .find(standing_in(state, tab))
                .map(|found| found.surface)
            else {
                return;
            };
            if state.window_rect(surface).is_some() {
                state.set_window_rect(surface, rect);
            }
        });
    }
}
