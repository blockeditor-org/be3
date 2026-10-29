use beui_view::types::{Direction, Pos2};
use serde::{Deserialize, Serialize};

use super::{DockState, TabId};

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
        self.bar_owner(bar)
            .is_some_and(|owner| self.is_shown(owner, stacked))
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
