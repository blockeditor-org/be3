use std::collections::{BTreeMap, BTreeSet, HashSet};

use be_model::{Anchor, Change, Document, Edit, List, Map, Model, ObjectId};
use logicgame::challenges::ChallengeId;
use logicgame::grid::{
    Component, ComponentId, ComponentKind, ComponentOrientation, LogicGrid, LogicGridSnapshot,
    Orientation, Point, Scale, Wire,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ChildChange, Root};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum LogicGridOperation {
    AddComponent {
        component: Component,
    },
    RemoveComponent {
        id: ComponentId,
    },
    MoveComponent {
        id: ComponentId,
        position: Point,
    },
    OrientComponent {
        id: ComponentId,
        orientation: ComponentOrientation,
    },
    SetComponentKind {
        id: ComponentId,
        kind: ComponentKind,
    },
    SetStorageValue {
        id: ComponentId,
        value: u64,
    },
    AddWire {
        wire: Wire,
    },
    RemoveWire {
        wire: Wire,
    },
    RemoveWireSegment {
        wire: Wire,
    },
    SetCompleted {
        completed: bool,
    },
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct LogicGridDocument {
    pub components: List<GridComponent>,
    pub wires: Map<Wire, ()>,
    pub challenge: Option<ChallengeId>,
    pub completed: bool,
    pub ends: Map<(Point, Scale), ()>,
}

#[derive(Clone, Debug, Default, Eq, Model, PartialEq)]
pub struct GridComponent {
    pub component: Option<ComponentId>,
    pub position: Option<Point>,
    pub orientation: Option<ComponentOrientation>,
    pub kind: Option<ComponentKind>,
}

impl GridComponent {
    fn of(component: &Component) -> Self {
        Self {
            component: Some(component.id),
            position: Some(component.position),
            orientation: Some(component.orientation),
            kind: Some(component.kind.clone()),
        }
    }

    fn component(&self) -> Option<Component> {
        Some(Component {
            id: self.component?,
            position: self.position?,
            orientation: self.orientation?,
            kind: self.kind.clone()?,
        })
    }
}

impl LogicGridDocument {
    pub fn with_grid(grid: &LogicGrid, challenge: Option<ChallengeId>) -> Self {
        Self {
            components: grid.components().map(GridComponent::of).collect(),
            wires: units(grid.wires())
                .into_iter()
                .map(|wire| (wire, ()))
                .collect(),
            challenge,
            completed: false,
            ends: ends(grid.wires())
                .into_iter()
                .map(|end| (end, ()))
                .collect(),
        }
    }

    pub fn for_challenge(challenge: ChallengeId) -> Self {
        Self {
            challenge: Some(challenge),
            ..Self::default()
        }
    }

    pub fn grid(&self) -> LogicGrid {
        LogicGrid::from_snapshot(LogicGridSnapshot {
            components: self
                .held_components()
                .into_iter()
                .map(|(_, component)| component)
                .collect(),
            wires: self.stored_wires(),
        })
    }

    fn held_components(&self) -> Vec<(ObjectId, Component)> {
        let mut next = self
            .components
            .iter()
            .filter_map(|held| held.component)
            .map(|id| id.0)
            .max()
            .map_or(0, |id| id.saturating_add(1));
        let mut seen = HashSet::new();
        self.components
            .iter()
            .filter_map(|held| {
                let mut component = held.component()?;
                if !seen.insert(component.id) {
                    component.id = ComponentId(next);
                    next = next.saturating_add(1);
                }
                Some((held.id, component))
            })
            .collect()
    }

    fn stored_wires(&self) -> Vec<Wire> {
        let mut runs = BTreeMap::<(Scale, Orientation, i64), Vec<(i64, i64)>>::new();
        for (wire, ()) in self.wires.iter() {
            let (fixed, start, end) = line(*wire);
            runs.entry((wire.scale, wire.orientation(), fixed))
                .or_default()
                .push((start, end));
        }
        let mut wires = Vec::new();
        for ((scale, orientation, fixed), mut spans) in runs {
            spans.sort_unstable();
            let mut joined: Vec<(i64, i64)> = Vec::new();
            for (start, end) in spans {
                match joined.last_mut() {
                    Some((_, last_end)) if start <= *last_end => *last_end = (*last_end).max(end),
                    _ => joined.push((start, end)),
                }
            }
            for (start, end) in joined {
                let point = |along: i64| match orientation {
                    Orientation::Horizontal => Point::new(along, fixed),
                    Orientation::Vertical => Point::new(fixed, along),
                };
                if end - start < scale.get() {
                    continue;
                }
                let mut cursor = start;
                for cut in (start + 1..end)
                    .filter(|along| self.ends.contains_key(&(point(*along), scale)))
                    .chain([end])
                {
                    wires.push(Wire {
                        start: point(cursor),
                        end: point(cut),
                        scale,
                    });
                    cursor = cut;
                }
            }
        }
        wires
    }

    pub fn called_blocks(&self) -> Vec<Uuid> {
        let mut seen = HashSet::new();
        self.components
            .iter()
            .filter_map(|held| match &held.kind {
                Some(ComponentKind::Subcomponent { compiled, .. }) => Some(*compiled),
                _ => None,
            })
            .filter(|compiled| seen.insert(*compiled))
            .collect()
    }

    fn holders(&self) -> BTreeMap<ComponentId, Vec<ObjectId>> {
        let mut holders = BTreeMap::<ComponentId, Vec<ObjectId>>::new();
        for (held, component) in self.held_components() {
            holders.entry(component.id).or_default().push(held);
        }
        holders
    }

    pub fn edit_for(&self, operation: &LogicGridOperation) -> Edit {
        self.edit_for_all(std::slice::from_ref(operation))
    }

    pub fn edit_for_all(&self, operations: &[LogicGridOperation]) -> Edit {
        let before = self.grid();
        let mut after = before.clone();
        let mut completed = None;
        for operation in operations {
            match operation {
                LogicGridOperation::AddComponent { component } => {
                    after.insert_component(component.clone());
                }
                LogicGridOperation::RemoveComponent { id } => {
                    after.remove_component(*id);
                }
                LogicGridOperation::MoveComponent { id, position } => {
                    after.set_component_position(*id, *position);
                }
                LogicGridOperation::OrientComponent { id, orientation } => {
                    after.set_component_orientation(*id, *orientation);
                }
                LogicGridOperation::SetComponentKind { id, kind } => {
                    after.set_component_kind(*id, kind.clone());
                }
                LogicGridOperation::SetStorageValue { id, value } => {
                    after.set_storage_value(*id, *value);
                }
                LogicGridOperation::AddWire { wire } => {
                    after.add_wire(*wire);
                }
                LogicGridOperation::RemoveWire { wire } => {
                    after.remove_wire(*wire);
                }
                LogicGridOperation::RemoveWireSegment { wire } => {
                    after.remove_wire_segment(*wire);
                }
                LogicGridOperation::SetCompleted { completed: value } => completed = Some(*value),
            }
        }
        let mut changes = self.component_changes(&before, &after);
        changes.extend(self.wire_changes(&after));
        if let Some(completed) = completed.filter(|value| *value != self.completed) {
            changes.push(Self::COMPLETED.set(ObjectId::ROOT, &completed));
        }
        Edit(changes)
    }

    fn component_changes(&self, before: &LogicGrid, after: &LogicGrid) -> Vec<Change> {
        let mut changes = Vec::new();
        let holders = self.holders();
        let holding = |id: ComponentId| holders.get(&id).cloned().unwrap_or_default();
        for component in before.components() {
            match after.component(component.id) {
                None => changes.extend(holding(component.id).into_iter().map(Change::remove)),
                Some(changed) if changed != component => {
                    for held in holding(component.id) {
                        if changed.position != component.position {
                            changes
                                .push(GridComponent::POSITION.set(held, &Some(changed.position)));
                        }
                        if changed.orientation != component.orientation {
                            changes.push(
                                GridComponent::ORIENTATION.set(held, &Some(changed.orientation)),
                            );
                        }
                        if changed.kind != component.kind {
                            changes
                                .push(GridComponent::KIND.set(held, &Some(changed.kind.clone())));
                        }
                    }
                }
                Some(_) => {}
            }
        }
        for component in after.components() {
            if before.component(component.id).is_none() {
                changes.push(
                    Self::COMPONENTS
                        .insert(ObjectId::ROOT, Anchor::End, &GridComponent::of(component))
                        .1,
                );
            }
        }
        changes
    }

    fn wire_changes(&self, after: &LogicGrid) -> Vec<Change> {
        let stored: BTreeSet<Wire> = self.wires.iter().map(|(wire, ())| *wire).collect();
        let wanted = units(after.wires());
        let stored_ends: BTreeSet<(Point, Scale)> =
            self.ends.iter().map(|(end, ())| *end).collect();
        let wanted_ends = ends(after.wires());
        let removed = stored
            .difference(&wanted)
            .map(|wire| Self::WIRES.put(ObjectId::ROOT, wire, None));
        let added = wanted
            .difference(&stored)
            .map(|wire| Self::WIRES.put(ObjectId::ROOT, wire, Some(&())));
        let ended = stored_ends
            .difference(&wanted_ends)
            .map(|end| Self::ENDS.put(ObjectId::ROOT, end, None));
        let started = wanted_ends
            .difference(&stored_ends)
            .map(|end| Self::ENDS.put(ObjectId::ROOT, end, Some(&())));
        removed.chain(added).chain(ended).chain(started).collect()
    }
}

fn line(wire: Wire) -> (i64, i64, i64) {
    match wire.orientation() {
        Orientation::Horizontal => (wire.start.y, wire.start.x, wire.end.x),
        Orientation::Vertical => (wire.start.x, wire.start.y, wire.end.y),
    }
}

fn units(wires: &[Wire]) -> BTreeSet<Wire> {
    let mut units = BTreeSet::new();
    for wire in wires {
        let (fixed, start, end) = line(*wire);
        for along in start..end {
            let (from, to) = match wire.orientation() {
                Orientation::Horizontal => (Point::new(along, fixed), Point::new(along + 1, fixed)),
                Orientation::Vertical => (Point::new(fixed, along), Point::new(fixed, along + 1)),
            };
            units.insert(Wire {
                start: from,
                end: to,
                scale: wire.scale,
            });
        }
    }
    units
}

fn ends(wires: &[Wire]) -> BTreeSet<(Point, Scale)> {
    wires
        .iter()
        .flat_map(|wire| [(wire.start, wire.scale), (wire.end, wire.scale)])
        .collect()
}

impl Root for LogicGridDocument {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x6c6f_6769_632d_6772_6964_2d62_6c6b_0101);

    fn references(&self) -> Vec<Uuid> {
        self.called_blocks()
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        match change {
            ChildChange::Add(_) => None,
            ChildChange::Delete(old) => Some(
                self.components
                    .iter()
                    .filter(|held| {
                        matches!(&held.kind, Some(ComponentKind::Subcomponent { compiled, .. }) if *compiled == old)
                    })
                    .map(|held| Change::remove(held.id))
                    .collect(),
            ),
            ChildChange::Replace { old, new } => Some(
                self.components
                    .iter()
                    .filter_map(|held| match &held.kind {
                        Some(ComponentKind::Subcomponent { compiled, .. }) if *compiled == old => {
                            let mut kind = held.kind.clone()?;
                            if let ComponentKind::Subcomponent { compiled, .. } = &mut kind {
                                *compiled = new;
                            }
                            Some(GridComponent::KIND.set(held.id, &Some(kind)))
                        }
                        _ => None,
                    })
                    .collect(),
            ),
        }
    }
}

pub type LogicGridContent = Document<LogicGridDocument>;
