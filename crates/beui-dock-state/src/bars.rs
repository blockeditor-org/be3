use beui_types::{Direction, Pos2};

use super::joined::joined_host;

const MAX_HOSTING_DEPTH: usize = 16;
use super::{DockState, Entry, GroupId, LeafId, TabId, Tree};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum DockAlign {
    Start,
    #[default]
    Center,
    End,
}

impl DockAlign {
    pub const ALL: [DockAlign; 3] = [DockAlign::Start, DockAlign::Center, DockAlign::End];

    pub fn side(self, edge: DockEdge) -> Option<&'static str> {
        match (self, edge.direction()) {
            (DockAlign::Center, _) => None,
            (DockAlign::Start, Direction::Horizontal) => Some("left"),
            (DockAlign::End, Direction::Horizontal) => Some("right"),
            (DockAlign::Start, Direction::Vertical) => Some("top"),
            (DockAlign::End, Direction::Vertical) => Some("bottom"),
        }
    }
}

/// `Over` floats over the body, which keeps its full size; `Beside` docks in a band along the whole
/// edge that the body gives up room for; `Window` pops out into a dock window (`origin` relative to
/// the dock), which floats over the body's edge in a stacked dock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockBarPlacement {
    Over(DockEdge, DockAlign),
    Beside(DockEdge, DockAlign),
    Window { edge: DockEdge, origin: Pos2 },
}

impl DockBarPlacement {
    pub fn edge(self) -> DockEdge {
        match self {
            DockBarPlacement::Over(edge, _)
            | DockBarPlacement::Beside(edge, _)
            | DockBarPlacement::Window { edge, .. } => edge,
        }
    }

    pub fn direction(self) -> Direction {
        self.edge().direction()
    }

    pub fn align(self) -> DockAlign {
        match self {
            DockBarPlacement::Over(_, align) | DockBarPlacement::Beside(_, align) => align,
            DockBarPlacement::Window { .. } => DockAlign::Center,
        }
    }

    pub fn is_beside(self) -> bool {
        matches!(self, DockBarPlacement::Beside(..))
    }

    pub fn is_window(self) -> bool {
        matches!(self, DockBarPlacement::Window { .. })
    }

    pub fn name(self) -> String {
        let (kind, edge, align) = match self {
            DockBarPlacement::Over(edge, align) => ("over", edge, align),
            DockBarPlacement::Beside(edge, align) => ("beside", edge, align),
            DockBarPlacement::Window { .. } => return "window".to_owned(),
        };
        match align.side(edge) {
            Some(side) => format!("{kind}.{}.{side}", edge.name()),
            None => format!("{kind}.{}", edge.name()),
        }
    }

    pub fn same_place(self, other: DockBarPlacement) -> bool {
        match (self, other) {
            (DockBarPlacement::Window { .. }, DockBarPlacement::Window { .. }) => true,
            _ => self == other,
        }
    }
}

/// `pane` is a tab of the joined dock its owner hosts: the bar shows on the pane that holds that
/// tab, else on the joined dock's first pane.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Bar {
    pub(super) bar: TabId,
    pub(super) owner: Option<TabId>,
    pub(super) placement: DockBarPlacement,
    pub(super) hidden: bool,
    pub(super) pane: Option<TabId>,
}

impl DockState {
    pub fn bars(&self) -> Vec<TabId> {
        self.bars.iter().map(|bar| bar.bar).collect()
    }

    pub fn bars_of(&self, owner: Option<TabId>) -> Vec<TabId> {
        self.bars
            .iter()
            .filter(|bar| bar.owner == owner)
            .map(|bar| bar.bar)
            .collect()
    }

    pub fn bar_owner(&self, bar: TabId) -> Option<TabId> {
        self.bar(bar)?.owner
    }

    pub fn is_dock_bar(&self, bar: TabId) -> bool {
        self.bar(bar).is_some_and(|bar| bar.owner.is_none())
    }

    pub fn bar_placement(&self, bar: TabId) -> Option<DockBarPlacement> {
        self.bar(bar).map(|bar| bar.placement)
    }

    pub fn set_bar_placement(&mut self, bar: TabId, placement: DockBarPlacement) {
        if let Some(held) = self.bars.iter_mut().find(|held| held.bar == bar) {
            held.placement = placement;
        }
    }

    pub fn is_bar_hidden(&self, bar: TabId) -> bool {
        self.bar(bar).is_some_and(|bar| bar.hidden)
    }

    pub fn set_bar_hidden(&mut self, bar: TabId, hidden: bool) {
        if let Some(held) = self.bars.iter_mut().find(|held| held.bar == bar) {
            held.hidden = hidden;
        }
    }

    pub fn add_bar(&mut self, bar: TabId, owner: Option<TabId>, placement: DockBarPlacement) {
        match self.bars.iter_mut().find(|held| held.bar == bar) {
            Some(held) => held.owner = owner,
            None => {
                let added = Bar {
                    bar,
                    owner,
                    placement,
                    hidden: false,
                    pane: None,
                };
                let at = self.layer_place(&added);
                self.bars.insert(at, added);
            }
        }
    }

    /// Where a bar new to a pane goes among the pane's bars: after the last of its own layer or a
    /// layer outside it, else before the layers inside it, so outer layers sit nearer the edge
    /// until someone drops a bar among another layer's.
    fn layer_place(&self, added: &Bar) -> usize {
        let Some((tab, depth)) = self.bar_pane(added) else {
            return self.bars.len();
        };
        let on_pane = |at: &usize| {
            self.bar_pane(&self.bars[*at])
                .is_some_and(|(shown, _)| shown == tab)
        };
        let depth_of = |at: usize| self.bar_pane(&self.bars[at]).map_or(0, |(_, held)| held);
        let mut held = (0..self.bars.len()).filter(on_pane);
        match (0..self.bars.len())
            .filter(on_pane)
            .rfind(|at| depth_of(*at) >= depth)
        {
            Some(last) => last + 1,
            None => held.next().unwrap_or(self.bars.len()),
        }
    }

    pub fn remove_bar(&mut self, bar: TabId) {
        self.bars.retain(|held| held.bar != bar);
    }

    pub fn is_bar_shown(&self, bar: TabId, mode: crate::DockMode) -> bool {
        if self.is_bar_hidden(bar) {
            return false;
        }
        if self.is_dock_bar(bar) {
            return true;
        }
        self.bar_tab(bar)
            .is_some_and(|tab| self.is_shown(tab, mode))
    }

    /// Whether the band of the dock's own bars along `edge` hides until its owner reveals it.
    pub fn is_edge_auto_hidden(&self, edge: DockEdge) -> bool {
        self.auto_hidden.contains(&edge)
    }

    pub fn set_edge_auto_hidden(&mut self, edge: DockEdge, hidden: bool) {
        self.auto_hidden.retain(|held| *held != edge);
        if hidden {
            self.auto_hidden.push(edge);
        }
    }

    /// The edges with a band of the dock's own bars, auto-hidden or not.
    pub fn dock_bands(&self) -> Vec<DockEdge> {
        DockEdge::ALL
            .into_iter()
            .filter(|edge| {
                self.bars.iter().any(|held| {
                    held.owner.is_none()
                        && !held.hidden
                        && held.placement.is_beside()
                        && held.placement.edge() == *edge
                })
            })
            .collect()
    }

    /// The edges whose band of the dock's own bars takes room from the body: not auto-hidden.
    pub fn dock_band_edges(&self) -> Vec<DockEdge> {
        DockEdge::ALL
            .into_iter()
            .filter(|edge| !self.auto_hidden.contains(edge))
            .filter(|edge| {
                self.bars.iter().any(|held| {
                    held.owner.is_none()
                        && !held.hidden
                        && held.placement.is_beside()
                        && held.placement.edge() == *edge
                })
            })
            .collect()
    }

    pub fn bar_tab(&self, bar: TabId) -> Option<TabId> {
        self.bar_pane(self.bar(bar)?).map(|(tab, _)| tab)
    }

    pub fn bar_depth(&self, bar: TabId) -> Option<usize> {
        if self.is_dock_bar(bar) {
            return Some(0);
        }
        self.bar_pane(self.bar(bar)?).map(|(_, depth)| depth)
    }

    /// The tabs whose panes a bar may show on: every pane of the joined dock its owner hosts (at
    /// any depth), else its owner's own pane. Empty for the dock's own bars.
    pub fn bar_panes(&self, bar: TabId) -> Vec<TabId> {
        let Some(owner) = self.bar_owner(bar) else {
            return Vec::new();
        };
        let Some(group) = self.hosted_group(owner) else {
            return self
                .owner_pane(owner)
                .map(|(tab, _)| tab)
                .into_iter()
                .collect();
        };
        let mut panes = Vec::new();
        self.group_panes(group, 0, &mut panes);
        panes
    }

    /// Shows a bar on the pane holding `pane`, one of `bar_panes`.
    pub fn set_bar_pane(&mut self, bar: TabId, pane: TabId) {
        let Some(owner) = self.bar_owner(bar) else {
            return;
        };
        let kept = self.pane_under(owner, pane).is_some().then_some(pane);
        if let Some(held) = self.bars.iter_mut().find(|held| held.bar == bar) {
            held.pane = kept;
        }
    }

    /// The bars a pane shows, in order from its edges: bars of every layer joined there, outer
    /// layers first unless a bar was dropped among another layer's.
    pub fn pane_bars(&self, tab: TabId) -> Vec<TabId> {
        self.bars
            .iter()
            .filter(|held| self.bar_pane(held).is_some_and(|(shown, _)| shown == tab))
            .map(|held| held.bar)
            .collect()
    }

    pub fn bars_with(&self, bar: TabId) -> Vec<TabId> {
        if self.is_dock_bar(bar) {
            return self.bars_of(None);
        }
        self.bar_tab(bar)
            .map(|tab| self.pane_bars(tab))
            .unwrap_or_default()
    }

    pub fn place_bar_at(&mut self, bar: TabId, placement: DockBarPlacement, slot: usize) {
        let Some(from) = self.bars.iter().position(|held| held.bar == bar) else {
            return;
        };
        let mut moved = self.bars.remove(from);
        moved.placement = placement;
        let pane = |held: &Bar| self.bar_pane(held).map(|(tab, _)| tab);
        let tab = pane(&moved);
        let siblings: Vec<usize> = self
            .bars
            .iter()
            .enumerate()
            .filter(|(_, held)| {
                held.owner.is_some() == moved.owner.is_some()
                    && held.placement.same_place(placement)
                    && pane(held) == tab
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

    pub fn bar_slot(&self, bar: TabId) -> usize {
        let Some(held) = self.bar(bar) else {
            return 0;
        };
        let pane = |held: &Bar| self.bar_pane(held).map(|(tab, _)| tab);
        let tab = pane(held);
        self.bars
            .iter()
            .take_while(|other| other.bar != bar)
            .filter(|other| {
                other.owner.is_some() == held.owner.is_some()
                    && other.placement.same_place(held.placement)
                    && pane(other) == tab
            })
            .count()
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

    /// The bar a key's shared window shows: the one whose tab was focused last and is shown. The other
    /// bars stay laid out in the window, clipped to nothing, so their bodies keep running.
    pub fn followed_bar(&self, bars: &[TabId]) -> Option<TabId> {
        let recent = self.recent_tabs();
        bars.iter()
            .copied()
            .filter(|bar| self.is_bar_shown(*bar, crate::DockMode::Tiled))
            .min_by_key(|bar| {
                self.bar_tab(*bar)
                    .and_then(|tab| recent.iter().position(|held| *held == tab))
                    .unwrap_or(usize::MAX)
            })
    }

    /// Places every bar of this bar's key the same way, so every instance's bar of one kind moves
    /// together.
    pub fn share_bar_placement(&mut self, bar: TabId, key: &dyn Fn(TabId) -> String) {
        let Some((placement, hidden)) = self.bar(bar).map(|bar| (bar.placement, bar.hidden)) else {
            return;
        };
        let shared = key(bar);
        for held in self.bars.iter_mut() {
            if held.bar != bar && key(held.bar) == shared {
                held.placement = placement;
                held.hidden = hidden;
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
                .map(|held| (held.placement, held.hidden));
            if let Some((placement, hidden)) = kept {
                self.set_bar_placement(bar, placement);
                self.set_bar_hidden(bar, hidden);
            }
        }
    }

    /// The tab whose pane shows a bar, and how many joined docks deep that pane is.
    fn bar_pane(&self, held: &Bar) -> Option<(TabId, usize)> {
        let owner = held.owner?;
        held.pane
            .and_then(|pane| self.pane_under(owner, pane))
            .or_else(|| self.owner_pane(owner))
    }

    fn owner_pane(&self, owner: TabId) -> Option<(TabId, usize)> {
        if self.contains(owner) {
            return Some((owner, 0));
        }
        let group = self.hosted_group(owner)?;
        let (tab, depth) = self.first_pane_tab(group, 0)?;
        Some((tab, depth + 1))
    }

    fn hosted_group(&self, owner: TabId) -> Option<GroupId> {
        self.groups
            .iter()
            .find(|group| group.key.as_deref().and_then(joined_host) == Some(owner))
            .map(|group| group.id)
    }

    fn is_joined(&self, group: GroupId) -> bool {
        self.group(group)
            .and_then(|held| held.key.as_deref())
            .and_then(joined_host)
            .is_some()
    }

    /// The pane holding `pane` inside the joined dock `owner` hosts: the tab it shows, and how
    /// many joined docks deep it is.
    fn pane_under(&self, owner: TabId, pane: TabId) -> Option<(TabId, usize)> {
        let group = self.hosted_group(owner)?;
        let leaf = self.find(pane)?.leaf;
        let mut depth = 0;
        let mut at = leaf;
        for _ in 0..=MAX_HOSTING_DEPTH {
            let Tree::Group(within) = self.tree_of(at)? else {
                return None;
            };
            depth += usize::from(self.is_joined(within));
            if within == group {
                return self.leaf_pane(leaf, depth);
            }
            at = self.holder(within)?.0;
        }
        None
    }

    fn leaf_pane(&self, leaf: LeafId, depth: usize) -> Option<(TabId, usize)> {
        match self.active_entry(leaf)? {
            Entry::Tab(tab) => Some((tab, depth)),
            Entry::Group(inner) => {
                let (tab, nested) = self.first_pane_tab(inner, 0)?;
                Some((tab, depth + nested + usize::from(self.is_joined(inner))))
            }
        }
    }

    fn group_panes(&self, group: GroupId, nested: usize, out: &mut Vec<TabId>) {
        if nested > MAX_HOSTING_DEPTH {
            return;
        }
        for leaf in self.tree_leaves(Tree::Group(group)) {
            match self.active_entry(leaf) {
                Some(Entry::Tab(tab)) => out.push(tab),
                Some(Entry::Group(inner)) => self.group_panes(inner, nested + 1, out),
                None => {}
            }
        }
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
