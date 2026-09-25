use std::collections::{BTreeSet, HashSet};

use be_model::{Anchor, Change, Document, Edit, List, Map, Model, ObjectId};
use logicgame::challenges::ChallengeId;
use logicgame::grid::{
    Component, ComponentId, ComponentKind, ComponentOrientation, LogicGrid, LogicGridSnapshot,
    Point, Wire,
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
            wires: grid.wires().iter().map(|wire| (*wire, ())).collect(),
            challenge,
            completed: false,
        }
    }

    pub fn for_challenge(challenge: ChallengeId) -> Self {
        Self {
            challenge: Some(challenge),
            ..Self::default()
        }
    }

    pub fn grid(&self) -> LogicGrid {
        let mut seen = HashSet::new();
        LogicGrid::from_snapshot(LogicGridSnapshot {
            components: self
                .components
                .iter()
                .filter_map(|held| held.component())
                .filter(|component| seen.insert(component.id))
                .collect(),
            wires: self.wires.iter().map(|(wire, ())| *wire).collect(),
        })
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

    fn holding(&self, id: ComponentId) -> impl Iterator<Item = &be_model::Item<GridComponent>> {
        self.components
            .iter()
            .filter(move |held| held.component == Some(id))
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
        for component in before.components() {
            match after.component(component.id) {
                None => changes.extend(
                    self.holding(component.id)
                        .map(|held| Change::remove(held.id)),
                ),
                Some(changed) if changed != component => {
                    for held in self.holding(component.id) {
                        if changed.position != component.position {
                            changes.push(
                                GridComponent::POSITION.set(held.id, &Some(changed.position)),
                            );
                        }
                        if changed.orientation != component.orientation {
                            changes.push(
                                GridComponent::ORIENTATION.set(held.id, &Some(changed.orientation)),
                            );
                        }
                        if changed.kind != component.kind {
                            changes.push(
                                GridComponent::KIND.set(held.id, &Some(changed.kind.clone())),
                            );
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
        let wanted: BTreeSet<Wire> = after.wires().iter().copied().collect();
        let removed = stored
            .difference(&wanted)
            .map(|wire| Self::WIRES.put(ObjectId::ROOT, wire, None));
        let added = wanted
            .difference(&stored)
            .map(|wire| Self::WIRES.put(ObjectId::ROOT, wire, Some(&())));
        removed.chain(added).collect()
    }
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
