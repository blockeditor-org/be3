use beui_view::types::{Direction, Pos2};
use serde::{Deserialize, Serialize};

use super::joined::joined_host;

const MAX_HOSTING_DEPTH: usize = 16;
use super::{DockState, Entry, GroupId, TabId, Tree};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub enum DockEdge {
    Top,
    Bottom,
    Left,
    Right,
}

impl DockEdge {
    pub const ALL: [DockEdge; 4] = [
        DockEdge::Top,
        DockEdge::Bottom,
        DockEdge::Left,
        DockEdge::Right,
    ];

    pub fn direction(self) -> Direction {
        match self {
            DockEdge::Top | DockEdge::Bottom => Direction::Horizontal,
            DockEdge::Left | DockEdge::Right => Direction::Vertical,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            DockEdge::Top => "top",
            DockEdge::Bottom => "bottom",
            DockEdge::Left => "left",
            DockEdge::Right => "right",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DockBarPlacement {
    Over(DockEdge),
    Beside(DockEdge),
    Window {
        edge: DockEdge,
        #[serde(with = "pos")]
        origin: Pos2,
    },
}

impl DockBarPlacement {
    pub fn edge(self) -> DockEdge {
        match self {
            DockBarPlacement::Over(edge)
            | DockBarPlacement::Beside(edge)
            | DockBarPlacement::Window { edge, .. } => edge,
        }
    }

    pub fn direction(self) -> Direction {
        self.edge().direction()
    }

    pub fn is_window(self) -> bool {
        matches!(self, DockBarPlacement::Window { .. })
    }

    pub fn same_place(self, other: DockBarPlacement) -> bool {
        match (self, other) {
            (DockBarPlacement::Window { .. }, DockBarPlacement::Window { .. }) => true,
            _ => self == other,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Bar {
    pub(super) bar: TabId,
    pub(super) owner: TabId,
    pub(super) placement: DockBarPlacement,
}

impl DockState {
    pub fn bars(&self) -> Vec<TabId> {
        self.bars.iter().map(|bar| bar.bar).collect()
    }

    pub fn bars_of(&self, owner: TabId) -> Vec<TabId> {
        self.bars
            .iter()
            .filter(|bar| bar.owner == owner)
            .map(|bar| bar.bar)
            .collect()
    }

    pub fn bar_owner(&self, bar: TabId) -> Option<TabId> {
        self.bar(bar).map(|bar| bar.owner)
    }

    pub fn bar_placement(&self, bar: TabId) -> Option<DockBarPlacement> {
        self.bar(bar).map(|bar| bar.placement)
    }

    pub fn set_bar_placement(&mut self, bar: TabId, placement: DockBarPlacement) {
        if let Some(held) = self.bars.iter_mut().find(|held| held.bar == bar) {
            held.placement = placement;
        }
    }

    pub fn add_bar(&mut self, bar: TabId, owner: TabId, placement: DockBarPlacement) {
        match self.bars.iter_mut().find(|held| held.bar == bar) {
            Some(held) => held.owner = owner,
            None => self.bars.push(Bar {
                bar,
                owner,
                placement,
            }),
        }
    }

    pub fn remove_bar(&mut self, bar: TabId) {
        self.bars.retain(|held| held.bar != bar);
    }

    pub fn is_bar_shown(&self, bar: TabId, stacked: bool) -> bool {
        self.bar_tab(bar)
            .is_some_and(|tab| self.is_shown(tab, stacked))
    }

    pub fn bar_tab(&self, bar: TabId) -> Option<TabId> {
        self.owner_pane(self.bar_owner(bar)?).map(|(tab, _)| tab)
    }

    pub fn bar_depth(&self, bar: TabId) -> Option<usize> {
        self.owner_pane(self.bar_owner(bar)?)
            .map(|(_, depth)| depth)
    }

    pub fn pane_bars(&self, tab: TabId) -> Vec<TabId> {
        let mut found: Vec<(usize, TabId)> = self
            .bars
            .iter()
            .filter_map(|held| {
                let (shown, depth) = self.owner_pane(held.owner)?;
                (shown == tab).then_some((depth, held.bar))
            })
            .collect();
        found.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));
        found.into_iter().map(|(_, bar)| bar).collect()
    }

    pub fn place_bar_at(&mut self, bar: TabId, placement: DockBarPlacement, slot: usize) {
        let Some(from) = self.bars.iter().position(|held| held.bar == bar) else {
            return;
        };
        let mut moved = self.bars.remove(from);
        moved.placement = placement;
        let tab = self.owner_pane(moved.owner).map(|(tab, _)| tab);
        let siblings: Vec<usize> = self
            .bars
            .iter()
            .enumerate()
            .filter(|(_, held)| {
                held.owner == moved.owner
                    && held.placement.same_place(placement)
                    && self.owner_pane(held.owner).map(|(tab, _)| tab) == tab
            })
            .map(|(at, _)| at)
            .collect();
        let at = match (siblings.get(slot), siblings.last()) {
            (Some(at), _) => *at,
            (None, Some(last)) => last + 1,
            (None, None) => from.min(self.bars.len()),
        };
        self.bars.insert(at, moved);
    }

    pub fn bar_windows(&self, key: &dyn Fn(TabId) -> String) -> Vec<(String, Vec<TabId>)> {
        let mut windows: Vec<(String, Vec<TabId>)> = Vec::new();
        for held in self.bars.iter().filter(|held| held.placement.is_window()) {
            let shared = key(held.bar);
            match windows.iter_mut().find(|(held, _)| *held == shared) {
                Some((_, bars)) => bars.push(held.bar),
                None => windows.push((shared, vec![held.bar])),
            }
        }
        windows
    }

    pub fn followed_bar(&self, bars: &[TabId]) -> Option<TabId> {
        let recent = self.recent_tabs();
        bars.iter()
            .copied()
            .filter(|bar| self.is_bar_shown(*bar, false))
            .min_by_key(|bar| {
                self.bar_tab(*bar)
                    .and_then(|tab| recent.iter().position(|held| *held == tab))
                    .unwrap_or(usize::MAX)
            })
    }

    pub fn share_bar_placement(&mut self, bar: TabId, key: &dyn Fn(TabId) -> String) {
        let Some(placement) = self.bar_placement(bar) else {
            return;
        };
        let shared = key(bar);
        for held in self.bars.iter_mut() {
            if held.bar != bar && key(held.bar) == shared {
                held.placement = placement;
            }
        }
    }

    pub fn adopt_bar_placements(&mut self, before: &DockState, key: &dyn Fn(TabId) -> String) {
        let arrived: Vec<TabId> = self
            .bars
            .iter()
            .map(|held| held.bar)
            .filter(|bar| before.bar(*bar).is_none())
            .collect();
        for bar in arrived {
            let shared = key(bar);
            let kept = before
                .bars
                .iter()
                .find(|held| self.bar(held.bar).is_some() && key(held.bar) == shared)
                .map(|held| held.placement);
            if let Some(placement) = kept {
                self.set_bar_placement(bar, placement);
            }
        }
    }

    fn owner_pane(&self, owner: TabId) -> Option<(TabId, usize)> {
        if self.contains(owner) {
            return Some((owner, 0));
        }
        let group = self
            .groups
            .iter()
            .find(|group| group.key.as_deref().and_then(joined_host) == Some(owner))?
            .id;
        let (tab, depth) = self.first_pane_tab(group, 0)?;
        Some((tab, depth + 1))
    }

    fn first_pane_tab(&self, group: GroupId, nested: usize) -> Option<(TabId, usize)> {
        if nested > MAX_HOSTING_DEPTH {
            return None;
        }
        let leaf = *self.tree_leaves(Tree::Group(group)).first()?;
        match self.active_entry(leaf)? {
            Entry::Tab(tab) => Some((tab, 0)),
            Entry::Group(inner) => {
                let (tab, depth) = self.first_pane_tab(inner, nested + 1)?;
                let hosted = self
                    .group(inner)
                    .and_then(|held| held.key.as_deref())
                    .and_then(joined_host);
                Some((tab, depth + usize::from(hosted.is_some())))
            }
        }
    }

    fn bar(&self, bar: TabId) -> Option<&Bar> {
        self.bars.iter().find(|held| held.bar == bar)
    }
}

mod pos {
    use beui_view::types::Pos2;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub(super) fn serialize<S: Serializer>(pos: &Pos2, serializer: S) -> Result<S::Ok, S::Error> {
        [pos.x, pos.y].serialize(serializer)
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Pos2, D::Error> {
        let [x, y] = <[f32; 2]>::deserialize(deserializer)?;
        Ok(Pos2::new(x, y))
    }
}
