use std::collections::{BTreeMap, HashSet};

use be_model::{Anchor, Change, Document, Edit, List, Map, Model, ObjectId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::database::DatabaseValue;
use crate::{BlockRef, ChildChange, Root};

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct CanvasPoint {
    pub x: f32,
    pub y: f32,
}

impl CanvasPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasTransform {
    pub center: CanvasPoint,
    pub size: CanvasPoint,
    pub rotation: f32,
}

impl CanvasTransform {
    pub const fn new(center: CanvasPoint, size: CanvasPoint, rotation: f32) -> Self {
        Self {
            center,
            size,
            rotation,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasPreviewRegion {
    pub center: CanvasPoint,
    pub size: CanvasPoint,
}

impl CanvasPreviewRegion {
    pub const fn new(center: CanvasPoint, size: CanvasPoint) -> Self {
        Self { center, size }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasEntityKind {
    Line,
    Rectangle,
    Text {
        text: String,
        text_style: CanvasTextStyle,
        placeholder: String,
    },
    Pen {
        points: Vec<CanvasPoint>,
    },
    Block {
        block_id: BlockRef,
    },
    DirectEditor {
        block_id: BlockRef,
        scale: f32,
    },
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTextWeight {
    #[default]
    Regular,
    Bold,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasTextAlign {
    #[default]
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasTextStyle {
    pub font_size: f32,
    pub weight: CanvasTextWeight,
    pub alignment: CanvasTextAlign,
    pub line_height: f32,
    pub wrap: bool,
}

impl Default for CanvasTextStyle {
    fn default() -> Self {
        Self {
            font_size: 18.0,
            weight: CanvasTextWeight::Regular,
            alignment: CanvasTextAlign::Left,
            line_height: 1.2,
            wrap: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasColor {
    #[default]
    Auto,
    Rgba {
        red: u8,
        green: u8,
        blue: u8,
        alpha: u8,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasEntityStyle {
    pub foreground: CanvasColor,
    pub line_width: f32,
    pub dashed: bool,
    pub fill: Option<CanvasColor>,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub corner_radius: f32,
    pub opacity: f32,
}

impl Default for CanvasEntityStyle {
    fn default() -> Self {
        Self {
            foreground: CanvasColor::Auto,
            line_width: 2.0,
            dashed: false,
            fill: None,
            arrow_start: false,
            arrow_end: false,
            corner_radius: 0.0,
            opacity: 1.0,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasComponent {
    pub schema_id: BlockRef,
    pub values: BTreeMap<Uuid, DatabaseValue>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CanvasEntity {
    pub id: Uuid,
    pub transform: CanvasTransform,
    pub kind: CanvasEntityKind,
    pub style: CanvasEntityStyle,
    pub group_id: Option<Uuid>,
    pub locked: bool,
    pub components: Vec<CanvasComponent>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasLayerMove {
    BringToFront,
    ForwardOne,
    BackOne,
    SendToBack,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum InfiniteCanvasOperation {
    Add {
        entity: CanvasEntity,
    },
    Update {
        entities: Vec<CanvasEntity>,
    },
    Remove {
        ids: Vec<Uuid>,
    },
    Reorder {
        ids: Vec<Uuid>,
        movement: CanvasLayerMove,
    },
    ExactOrder {
        ids: Vec<Uuid>,
    },
    SetPreviewRegion {
        region: Option<CanvasPreviewRegion>,
    },
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Canvas {
    pub entities: List<Entity>,
    pub preview_region: Option<CanvasPreviewRegion>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Entity {
    pub entity: Uuid,
    pub center: CanvasPoint,
    pub size: CanvasPoint,
    pub rotation: f32,
    pub kind: Option<CanvasEntityKind>,
    pub style: CanvasEntityStyle,
    pub group_id: Option<Uuid>,
    pub locked: bool,
    pub components: List<Component>,
}

#[derive(Clone, Debug, Default, Model, PartialEq)]
pub struct Component {
    pub schema: Option<BlockRef>,
    pub values: Map<Uuid, DatabaseValue>,
}

impl Entity {
    fn of(entity: &CanvasEntity) -> Self {
        Self {
            entity: entity.id,
            center: entity.transform.center,
            size: entity.transform.size,
            rotation: entity.transform.rotation,
            kind: Some(entity.kind.clone()),
            style: entity.style,
            group_id: entity.group_id,
            locked: entity.locked,
            components: entity.components.iter().map(Component::of).collect(),
        }
    }

    fn view(&self) -> Option<CanvasEntity> {
        Some(CanvasEntity {
            id: self.entity,
            transform: CanvasTransform::new(self.center, self.size, self.rotation),
            kind: self.kind.clone()?,
            style: self.style,
            group_id: self.group_id,
            locked: self.locked,
            components: self
                .components
                .iter()
                .filter_map(|component| component.view())
                .collect(),
        })
    }
}

impl Component {
    fn of(component: &CanvasComponent) -> Self {
        Self {
            schema: Some(component.schema_id),
            values: component
                .values
                .iter()
                .map(|(field, value)| (*field, value.clone()))
                .collect(),
        }
    }

    fn view(&self) -> Option<CanvasComponent> {
        Some(CanvasComponent {
            schema_id: self.schema?,
            values: self
                .values
                .iter()
                .map(|(field, value)| (*field, value.clone()))
                .collect(),
        })
    }
}

impl Canvas {
    pub fn with_entities(
        entities: impl IntoIterator<Item = CanvasEntity>,
        preview_region: Option<CanvasPreviewRegion>,
    ) -> Self {
        Self {
            entities: entities
                .into_iter()
                .map(|entity| Entity::of(&normalized_entity(entity)))
                .collect(),
            preview_region: preview_region.map(normalized_preview_region),
        }
    }

    pub fn entities(&self) -> Vec<CanvasEntity> {
        let mut seen = HashSet::new();
        self.entities
            .iter()
            .filter(|held| seen.insert(held.entity))
            .filter_map(|held| held.view())
            .collect()
    }

    fn holding(&self, id: Uuid) -> impl Iterator<Item = &be_model::Item<Entity>> {
        self.entities.iter().filter(move |held| held.entity == id)
    }

    pub fn edit_for(&self, operation: &InfiniteCanvasOperation) -> Edit {
        match operation {
            InfiniteCanvasOperation::Add { entity } => {
                if self.holding(entity.id).next().is_some() {
                    return Edit::default();
                }
                Self::ENTITIES
                    .insert(
                        ObjectId::ROOT,
                        Anchor::End,
                        &Entity::of(&normalized_entity(entity.clone())),
                    )
                    .1
                    .into()
            }
            InfiniteCanvasOperation::Update { entities } => entities
                .iter()
                .flat_map(|update| {
                    let update = normalized_entity(update.clone());
                    self.holding(update.id)
                        .flat_map(|held| update_entity(held, &update))
                        .collect::<Vec<_>>()
                })
                .collect(),
            InfiniteCanvasOperation::Remove { ids } => self
                .entities
                .iter()
                .filter(|held| ids.contains(&held.entity))
                .map(|held| Change::remove(held.id))
                .collect(),
            InfiniteCanvasOperation::Reorder { ids, movement } => {
                let order: Vec<Uuid> = self.entities.iter().map(|held| held.entity).collect();
                self.reorder_to(&reordered(&order, ids, *movement), ids)
            }
            InfiniteCanvasOperation::ExactOrder { ids } => {
                let order: Vec<Uuid> = self.entities.iter().map(|held| held.entity).collect();
                self.reorder_to(&exactly_ordered(&order, ids), ids)
            }
            InfiniteCanvasOperation::SetPreviewRegion { region } => Self::PREVIEW_REGION
                .set(ObjectId::ROOT, &region.map(normalized_preview_region))
                .into(),
        }
    }

    fn reorder_to(&self, target: &[Uuid], moved: &[Uuid]) -> Edit {
        let objects: Vec<(Uuid, ObjectId)> = self
            .entities
            .iter()
            .map(|held| (held.entity, held.id))
            .collect();
        let current: Vec<Uuid> = objects.iter().map(|(entity, _)| *entity).collect();
        if current == target {
            return Edit::default();
        }
        let object_of = |entity: Uuid| {
            objects
                .iter()
                .find(|(held, _)| *held == entity)
                .map(|(_, object)| *object)
        };
        let mut changes = Vec::new();
        for (index, entity) in target.iter().enumerate() {
            if !moved.contains(entity) {
                continue;
            }
            let Some(object) = object_of(*entity) else {
                continue;
            };
            let anchor = match index
                .checked_sub(1)
                .and_then(|previous| object_of(target[previous]))
            {
                Some(previous) => Anchor::After(previous),
                None => Anchor::Start,
            };
            changes.push(Self::ENTITIES.move_into(ObjectId::ROOT, anchor, object));
        }
        Edit(changes)
    }

    fn child_entities(&self, old: BlockRef) -> impl Iterator<Item = &be_model::Item<Entity>> {
        self.entities.iter().filter(move |held| {
            matches!(
                held.kind,
                Some(
                    CanvasEntityKind::Block { block_id }
                        | CanvasEntityKind::DirectEditor { block_id, .. }
                ) if block_id == old
            )
        })
    }
}

fn update_entity(held: &be_model::Item<Entity>, update: &CanvasEntity) -> Vec<Change> {
    let id = held.id;
    let mut changes = Vec::new();
    if held.center != update.transform.center {
        changes.push(Entity::CENTER.set(id, &update.transform.center));
    }
    if held.size != update.transform.size {
        changes.push(Entity::SIZE.set(id, &update.transform.size));
    }
    if held.rotation != update.transform.rotation {
        changes.push(Entity::ROTATION.set(id, &update.transform.rotation));
    }
    if held.kind.as_ref() != Some(&update.kind) {
        changes.push(Entity::KIND.set(id, &Some(update.kind.clone())));
    }
    if held.style != update.style {
        changes.push(Entity::STYLE.set(id, &update.style));
    }
    if held.group_id != update.group_id {
        changes.push(Entity::GROUP_ID.set(id, &update.group_id));
    }
    if held.locked != update.locked {
        changes.push(Entity::LOCKED.set(id, &update.locked));
    }
    changes.extend(update_components(held, &update.components));
    changes
}

fn update_components(held: &be_model::Item<Entity>, wanted: &[CanvasComponent]) -> Vec<Change> {
    let mut changes = Vec::new();
    for component in held.components.iter() {
        let Some(update) = wanted
            .iter()
            .find(|wanted| Some(wanted.schema_id) == component.schema)
        else {
            changes.push(Change::remove(component.id));
            continue;
        };
        for (field, value) in component.values.iter() {
            if !update.values.contains_key(field) {
                changes.push(Component::VALUES.put(component.id, field, None));
            } else if update.values.get(field) != Some(value) {
                changes.push(Component::VALUES.put(component.id, field, update.values.get(field)));
            }
        }
        for (field, value) in &update.values {
            if !component.values.contains_key(field) {
                changes.push(Component::VALUES.put(component.id, field, Some(value)));
            }
        }
    }
    for component in wanted {
        if !held
            .components
            .iter()
            .any(|existing| existing.schema == Some(component.schema_id))
        {
            changes.push(
                Entity::COMPONENTS
                    .insert(held.id, Anchor::End, &Component::of(component))
                    .1,
            );
        }
    }
    changes
}

fn reordered(order: &[Uuid], ids: &[Uuid], movement: CanvasLayerMove) -> Vec<Uuid> {
    let selected: HashSet<_> = ids.iter().copied().collect();
    let mut order = order.to_vec();
    match movement {
        CanvasLayerMove::BringToFront => {
            let (mut back, front): (Vec<_>, Vec<_>) =
                order.into_iter().partition(|id| !selected.contains(id));
            back.extend(front);
            order = back;
        }
        CanvasLayerMove::ForwardOne => {
            for index in (0..order.len().saturating_sub(1)).rev() {
                if selected.contains(&order[index]) && !selected.contains(&order[index + 1]) {
                    order.swap(index, index + 1);
                }
            }
        }
        CanvasLayerMove::BackOne => {
            for index in 1..order.len() {
                if selected.contains(&order[index]) && !selected.contains(&order[index - 1]) {
                    order.swap(index - 1, index);
                }
            }
        }
        CanvasLayerMove::SendToBack => {
            let (mut back, front): (Vec<_>, Vec<_>) =
                order.into_iter().partition(|id| selected.contains(id));
            back.extend(front);
            order = back;
        }
    }
    order
}

fn exactly_ordered(order: &[Uuid], ids: &[Uuid]) -> Vec<Uuid> {
    let mut ranked: Vec<Uuid> = order
        .iter()
        .filter(|id| ids.contains(id))
        .copied()
        .collect();
    ranked.sort_by_key(|entity| ids.iter().position(|id| id == entity).unwrap_or(usize::MAX));
    let mut ranked = ranked.into_iter();
    order
        .iter()
        .map(|id| match ids.contains(id) {
            true => ranked.next().unwrap_or(*id),
            false => *id,
        })
        .collect()
}

impl Root for Canvas {
    const CONTENT_TYPE: Uuid = Uuid::from_u128(0x696e_6669_6e69_7465_2d63_616e_7661_0002);

    fn references(&self) -> Vec<Uuid> {
        let mut references = Vec::new();
        for held in self.entities.iter() {
            if let Some(
                CanvasEntityKind::Block { block_id }
                | CanvasEntityKind::DirectEditor { block_id, .. },
            ) = &held.kind
            {
                references.extend(block_id.as_direct());
            }
            for component in held.components.iter() {
                references.extend(component.schema.and_then(|schema| schema.as_direct()));
                references.extend(component.values.values().filter_map(|value| match value {
                    DatabaseValue::Block(reference) => reference.as_direct(),
                    _ => None,
                }));
            }
        }
        let mut seen = HashSet::new();
        references.retain(|reference| seen.insert(*reference));
        references
    }

    fn child_edit(&self, change: ChildChange) -> Option<Edit> {
        match change {
            ChildChange::Add(_) => None,
            ChildChange::Delete(old) => {
                let old = BlockRef::Direct(old);
                let mut changes: Vec<Change> = self
                    .child_entities(old)
                    .map(|held| Change::remove(held.id))
                    .collect();
                for held in self.entities.iter() {
                    for component in held.components.iter() {
                        if component.schema == Some(old) {
                            changes.push(Change::remove(component.id));
                            continue;
                        }
                        for (field, value) in component.values.iter() {
                            if *value == DatabaseValue::Block(old) {
                                changes.push(Component::VALUES.put(component.id, field, None));
                            }
                        }
                    }
                }
                Some(Edit(changes))
            }
            ChildChange::Replace { old, new } => {
                let (old, new) = (BlockRef::Direct(old), BlockRef::Direct(new));
                let mut changes = Vec::new();
                for held in self.child_entities(old) {
                    let kind = match held.kind.clone()? {
                        CanvasEntityKind::Block { .. } => CanvasEntityKind::Block { block_id: new },
                        CanvasEntityKind::DirectEditor { scale, .. } => {
                            CanvasEntityKind::DirectEditor {
                                block_id: new,
                                scale,
                            }
                        }
                        other => other,
                    };
                    changes.push(Entity::KIND.set(held.id, &Some(kind)));
                }
                for held in self.entities.iter() {
                    let existing = held
                        .components
                        .iter()
                        .find(|component| component.schema == Some(new));
                    for component in held.components.iter() {
                        if component.schema == Some(old) {
                            match existing {
                                Some(existing) => {
                                    for (field, value) in component.values.iter() {
                                        if !existing.values.contains_key(field) {
                                            changes.push(Component::VALUES.put(
                                                existing.id,
                                                field,
                                                Some(value),
                                            ));
                                        }
                                    }
                                    changes.push(Change::remove(component.id));
                                }
                                None => {
                                    changes.push(Component::SCHEMA.set(component.id, &Some(new)));
                                }
                            }
                        }
                        for (field, value) in component.values.iter() {
                            if *value == DatabaseValue::Block(old) {
                                changes.push(Component::VALUES.put(
                                    component.id,
                                    field,
                                    Some(&DatabaseValue::Block(new)),
                                ));
                            }
                        }
                    }
                }
                Some(Edit(changes))
            }
        }
    }
}

pub fn normalized_entity(mut entity: CanvasEntity) -> CanvasEntity {
    if let CanvasEntityKind::DirectEditor { scale, .. } = &mut entity.kind {
        entity.transform.rotation = 0.0;
        if !scale.is_finite() || *scale <= 0.0 {
            *scale = 1.0;
        }
    }
    entity
}

pub fn normalized_preview_region(mut region: CanvasPreviewRegion) -> CanvasPreviewRegion {
    if !region.center.x.is_finite() {
        region.center.x = 0.0;
    }
    if !region.center.y.is_finite() {
        region.center.y = 0.0;
    }
    if !region.size.x.is_finite() {
        region.size.x = 100.0;
    }
    if !region.size.y.is_finite() {
        region.size.y = 100.0;
    }
    region.size.x = region.size.x.abs().max(1.0);
    region.size.y = region.size.y.abs().max(1.0);
    region
}

pub type CanvasContent = Document<Canvas>;
