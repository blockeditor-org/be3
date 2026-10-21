use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

use block_editor_beui::ContentProjection;
use block_editor_beui::be_block::be_model::Place;
use block_editor_beui::be_block::canvas::{
    Canvas, CanvasEntity, CanvasPoint, CanvasTransform, Entity,
};
use block_editor_beui::be_block::{CanvasContent, ObjectId, Touched};
use block_editor_beui::beui::reactive::*;
use uuid::Uuid;

use crate::geometry::{WorldRect, entity_bounds};

#[derive(Clone, Copy)]
pub(crate) struct EntitySignals {
    held: Memo<Option<CanvasEntity>>,
    held_shown: Memo<Option<CanvasEntity>>,
    stored: Memo<Option<CanvasEntity>>,
    shown: Memo<Option<CanvasEntity>>,
    frame: Memo<bool>,
    embeds: Memo<bool>,
}

impl EntitySignals {
    pub(crate) fn stored(&self) -> Memo<Option<CanvasEntity>> {
        self.stored
    }

    pub(crate) fn shown(&self) -> Memo<Option<CanvasEntity>> {
        self.shown
    }
}

struct Slot {
    signals: EntitySignals,
    preview: Signal<Option<CanvasEntity>>,
    watchers: usize,
    _scope: Scope,
}

struct Slots {
    content: ContentProjection<CanvasContent>,
    owner: ScopeContext,
    slots: RefCell<HashMap<Uuid, Slot>>,
    released: RefCell<HashSet<Uuid>>,
    previewed: RefCell<HashSet<Uuid>>,
    catalog: RefCell<Catalog>,
}

#[derive(Clone, Copy)]
pub(crate) struct Entities {
    slots: Stored<Slots>,
    ids: Memo<Vec<Uuid>>,
    previewed: Signal<Vec<Uuid>>,
    shown_ids: Memo<Vec<Uuid>>,
    frames: Signal<Vec<Uuid>>,
    embedding: Signal<Vec<Uuid>>,
    extent: Signal<Option<WorldRect>>,
}

struct Listed {
    frame: bool,
    embeds: bool,
    bounds: WorldRect,
    targets: Vec<Uuid>,
}

#[derive(Clone, Copy, PartialEq)]
struct Edge(f32);

impl Eq for Edge {}

impl PartialOrd for Edge {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Edge {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Default)]
struct Edges([BTreeMap<Edge, usize>; 4]);

impl Edges {
    fn sides(bounds: WorldRect) -> [Edge; 4] {
        [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y].map(Edge)
    }

    fn add(&mut self, bounds: WorldRect) {
        for (side, edge) in self.0.iter_mut().zip(Self::sides(bounds)) {
            *side.entry(edge).or_default() += 1;
        }
    }

    fn remove(&mut self, bounds: WorldRect) {
        for (side, edge) in self.0.iter_mut().zip(Self::sides(bounds)) {
            if let Some(count) = side.get_mut(&edge) {
                *count -= 1;
                if *count == 0 {
                    side.remove(&edge);
                }
            }
        }
    }

    fn extent(&self) -> Option<WorldRect> {
        let [left, top, right, bottom] = &self.0;
        Some(WorldRect {
            min: CanvasPoint::new(left.first_key_value()?.0.0, top.first_key_value()?.0.0),
            max: CanvasPoint::new(right.last_key_value()?.0.0, bottom.last_key_value()?.0.0),
        })
    }
}

#[derive(Default)]
struct Catalog {
    listed: HashMap<Uuid, Listed>,
    bound_by: HashMap<Uuid, HashSet<Uuid>>,
    edges: Edges,
    framing: HashSet<Uuid>,
    embedders: HashSet<Uuid>,
    frames: Vec<Uuid>,
    embedding: Vec<Uuid>,
    visited: usize,
}

struct Catalogued {
    removed: Vec<Uuid>,
    frames: Vec<Uuid>,
    embedding: Vec<Uuid>,
    extent: Option<WorldRect>,
}

const LIST: Place = Place {
    object: ObjectId::ROOT,
    field: Canvas::ENTITIES.index(),
};

fn outline(content: &CanvasContent, id: Uuid) -> Option<CanvasEntity> {
    let object = ObjectId::from_uuid(id);
    if content.tree().object(object)?.parent() != Some(LIST) {
        return None;
    }
    let transform = CanvasTransform::new(
        content.field(object, Entity::CENTER),
        content.field(object, Entity::SIZE),
        content.field(object, Entity::ROTATION),
    );
    Some(CanvasEntity {
        id,
        transform,
        shape: content.field(object, Entity::SHAPE)?,
        points: content.field(object, Entity::POINTS),
        bindings: [
            content.field(object, Entity::START_BINDING),
            content.field(object, Entity::END_BINDING),
        ],
        block: None,
        scale: 1.0,
        pressures: Vec::new(),
        text: Default::default(),
        placeholder: String::new(),
        text_style: Default::default(),
        style: Default::default(),
        hidden: false,
        group_id: None,
        locked: false,
        components: Vec::new(),
    })
}

impl Catalog {
    fn refresh(&mut self, content: &CanvasContent, touched: &[Touched]) -> Catalogued {
        let everything = touched.contains(&Touched::Everything);
        let mut changed: HashSet<Uuid> = match everything {
            true => content
                .ids(ObjectId::ROOT, Canvas::ENTITIES)
                .into_iter()
                .map(ObjectId::as_uuid)
                .chain(self.listed.keys().copied())
                .collect(),
            false => touched
                .iter()
                .filter_map(|touched| match touched {
                    Touched::Subtree(id) if *id != ObjectId::ROOT => Some(id.as_uuid()),
                    _ => None,
                })
                .collect(),
        };
        let followers: Vec<Uuid> = changed
            .iter()
            .filter_map(|id| self.bound_by.get(id))
            .flatten()
            .copied()
            .collect();
        changed.extend(followers);
        let mut kinds_changed = everything;
        let mut removed = Vec::new();
        for id in &changed {
            let was = self.listed.remove(id);
            self.framing.remove(id);
            self.embedders.remove(id);
            if let Some(was) = &was {
                self.edges.remove(was.bounds);
                for target in &was.targets {
                    if let Some(followers) = self.bound_by.get_mut(target) {
                        followers.remove(id);
                        if followers.is_empty() {
                            self.bound_by.remove(target);
                        }
                    }
                }
            }
            let Some(mut entity) = outline(content, *id) else {
                kinds_changed |= was.is_some();
                if was.is_some() {
                    removed.push(*id);
                }
                continue;
            };
            self.visited += 1;
            entity.follow(|target| outline(content, target));
            let listed = Listed {
                frame: entity.is_frame(),
                embeds: entity.shape.embeds_block(),
                bounds: entity_bounds(&entity),
                targets: entity
                    .bindings
                    .iter()
                    .flatten()
                    .map(|binding| binding.entity)
                    .collect(),
            };
            kinds_changed |=
                was.is_none_or(|was| (was.frame, was.embeds) != (listed.frame, listed.embeds));
            if listed.frame {
                self.framing.insert(*id);
            }
            if listed.embeds {
                self.embedders.insert(*id);
            }
            self.edges.add(listed.bounds);
            for target in &listed.targets {
                self.bound_by.entry(*target).or_default().insert(*id);
            }
            self.listed.insert(*id, listed);
        }
        let reordered = touched.contains(&Touched::Field(ObjectId::ROOT, LIST.field));
        if kinds_changed || reordered {
            let order = |ids: &HashSet<Uuid>| {
                let mut ids: Vec<(usize, Uuid)> = ids
                    .iter()
                    .filter_map(|id| {
                        let index = content.index_of(
                            ObjectId::ROOT,
                            Canvas::ENTITIES,
                            ObjectId::from_uuid(*id),
                        )?;
                        Some((index, *id))
                    })
                    .collect();
                ids.sort_unstable();
                ids.into_iter().map(|(_, id)| id).collect::<Vec<_>>()
            };
            self.frames = order(&self.framing);
            self.embedding = order(&self.embedders);
        }
        Catalogued {
            removed,
            frames: self.frames.clone(),
            embedding: self.embedding.clone(),
            extent: self.edges.extent(),
        }
    }
}

impl Entities {
    pub(crate) fn new(content: ContentProjection<CanvasContent>) -> Self {
        let objects = content.ids(ObjectId::ROOT, Canvas::ENTITIES);
        let ids = create_memo(move || {
            objects.with(|ids| ids.iter().map(|id| id.as_uuid()).collect::<Vec<_>>())
        });
        let previewed = create_signal(Vec::<Uuid>::new());
        let shown_ids = create_memo(move || {
            ids.with(|stored| {
                previewed.with(|previewed| {
                    if previewed.is_empty() {
                        return stored.clone();
                    }
                    let held: HashSet<&Uuid> = stored.iter().collect();
                    let added: Vec<Uuid> = previewed
                        .iter()
                        .filter(|id| !held.contains(id))
                        .copied()
                        .collect();
                    stored.iter().copied().chain(added).collect()
                })
            })
        });
        let slots = store(Slots {
            content,
            owner: owner_scope().expect("the canvas entities are made in a scope"),
            slots: RefCell::new(HashMap::new()),
            released: RefCell::new(HashSet::new()),
            previewed: RefCell::new(HashSet::new()),
            catalog: RefCell::new(Catalog::default()),
        });
        let entities = Self {
            slots,
            ids,
            previewed,
            shown_ids,
            frames: create_signal(Vec::new()),
            embedding: create_signal(Vec::new()),
            extent: create_signal(None),
        };
        content.on_touched(Touched::Subtree(ObjectId::ROOT), move |content, touched| {
            let Some(slots) = entities.slots.try_value() else {
                return;
            };
            let catalogued = slots.catalog.borrow_mut().refresh(content, touched);
            slots.released.borrow_mut().extend(catalogued.removed);
            if entities
                .frames
                .with_untracked(|held| *held != catalogued.frames)
            {
                entities.frames.set(catalogued.frames);
            }
            if entities
                .embedding
                .with_untracked(|held| *held != catalogued.embedding)
            {
                entities.embedding.set(catalogued.embedding);
            }
            if entities.extent.get_untracked() != catalogued.extent {
                entities.extent.set(catalogued.extent);
            }
        });
        create_effect(move || {
            ids.with(|_| ());
            untrack(|| entities.sweep());
        });
        entities
    }

    pub(crate) fn ids(&self) -> Memo<Vec<Uuid>> {
        self.ids
    }

    pub(crate) fn shown_ids(&self) -> Memo<Vec<Uuid>> {
        self.shown_ids
    }

    pub(crate) fn track_edits(&self) {
        let _ = self.slots.value().content.revision();
    }

    pub(crate) fn watch(&self, id: Uuid) -> EntitySignals {
        let signals = self.signals(id);
        let slots = self.slots.value();
        if let Some(slot) = slots.slots.borrow_mut().get_mut(&id) {
            slot.watchers += 1;
        }
        let entities = *self;
        on_cleanup(move || entities.unwatch(id));
        signals
    }

    pub(crate) fn get(&self, id: Uuid) -> Option<CanvasEntity> {
        self.ids.with(|_| ());
        self.signals(id).stored.get()
    }

    pub(crate) fn get_untracked(&self, id: Uuid) -> Option<CanvasEntity> {
        untrack(|| self.get(id))
    }

    pub(crate) fn shown(&self, id: Uuid) -> Option<CanvasEntity> {
        self.shown_ids.with(|_| ());
        self.signals(id).shown.get()
    }

    pub(crate) fn all(&self) -> Vec<CanvasEntity> {
        self.ids.with(|ids| {
            ids.iter()
                .filter_map(|id| self.signals(*id).stored.get())
                .collect()
        })
    }

    pub(crate) fn all_untracked(&self) -> Vec<CanvasEntity> {
        untrack(|| self.all())
    }

    pub(crate) fn among(&self, selection: &HashSet<Uuid>) -> Vec<CanvasEntity> {
        self.ids.with(|ids| {
            ids.iter()
                .filter(|id| selection.contains(id))
                .filter_map(|id| self.signals(*id).stored.get())
                .collect()
        })
    }

    pub(crate) fn shown_among(&self, selection: &HashSet<Uuid>) -> Vec<CanvasEntity> {
        self.shown_ids.with(|ids| {
            ids.iter()
                .filter(|id| selection.contains(id))
                .filter_map(|id| self.signals(*id).shown.get())
                .collect()
        })
    }

    pub(crate) fn shown_embedding(&self) -> Vec<Uuid> {
        self.shown_of(self.embedding, |signals| signals.embeds)
            .into_iter()
            .map(|(id, _)| id)
            .collect()
    }

    pub(crate) fn first_frame(&self) -> Option<CanvasEntity> {
        let first = self.frames.with(|frames| frames.first().copied())?;
        self.signals(first).stored.get()
    }

    pub(crate) fn shown_frames(&self) -> Vec<CanvasEntity> {
        self.shown_of(self.frames, |signals| signals.frame)
            .into_iter()
            .filter_map(|(_, signals)| signals.shown.get())
            .collect()
    }

    pub(crate) fn embedding_untracked(&self) -> Vec<Uuid> {
        self.embedding.get_untracked()
    }

    pub(crate) fn region(&self) -> WorldRect {
        crate::geometry::embedded_region(
            self.first_frame().as_ref().map(entity_bounds),
            self.extent.get(),
        )
    }

    #[cfg(test)]
    pub(crate) fn catalogued(&self) -> usize {
        self.slots.value().catalog.borrow().visited
    }

    fn shown_of(
        &self,
        stored: Signal<Vec<Uuid>>,
        kind: fn(&EntitySignals) -> Memo<bool>,
    ) -> Vec<(Uuid, EntitySignals)> {
        stored.with(|stored| {
            self.previewed.with(|previewed| {
                let held: HashSet<&Uuid> = stored.iter().collect();
                stored
                    .iter()
                    .chain(previewed.iter().filter(|id| !held.contains(id)))
                    .map(|id| (*id, self.signals(*id)))
                    .filter(|(_, signals)| kind(signals).get())
                    .collect()
            })
        })
    }

    fn followed(
        &self,
        mut entity: CanvasEntity,
        target: fn(EntitySignals) -> Memo<Option<CanvasEntity>>,
    ) -> CanvasEntity {
        if entity.bindings.iter().any(Option::is_some) {
            entity.follow(|id| target(self.watch(id)).get());
        }
        entity
    }

    pub(crate) fn preview(&self, previews: Vec<CanvasEntity>) {
        let slots = self.slots.value();
        let ids: Vec<Uuid> = previews.iter().map(|entity| entity.id).collect();
        let next: HashSet<Uuid> = ids.iter().copied().collect();
        let gone: Vec<Uuid> = slots
            .previewed
            .replace(next)
            .into_iter()
            .filter(|id| !ids.contains(id))
            .collect();
        for entity in previews {
            let preview = self.slot_preview(entity.id);
            preview.set(Some(entity));
        }
        for id in &gone {
            if let Some(preview) = slots.slots.borrow().get(id).map(|slot| slot.preview) {
                preview.set(None);
            }
        }
        self.previewed.set(ids);
        self.release(&slots, &gone);
    }

    fn slot_preview(&self, id: Uuid) -> Signal<Option<CanvasEntity>> {
        self.signals(id);
        self.slots.value().slots.borrow()[&id].preview
    }

    fn signals(&self, id: Uuid) -> EntitySignals {
        let slots = self.slots.value();
        if let Some(slot) = slots.slots.borrow().get(&id) {
            return slot.signals;
        }
        let scope = slots
            .owner
            .child()
            .expect("the canvas outlives its entities");
        let content = slots.content;
        let entities = *self;
        let (signals, preview) = untrack(|| {
            scope.run(|| {
                let object = ObjectId::from_uuid(id);
                let object_held = content.object::<Entity>(object);
                let preview = create_signal(None::<CanvasEntity>);
                let held =
                    create_memo(move || object_held.with(|entity| entity.as_ref()?.view(object)));
                let held_shown = create_memo(move || preview.get().or_else(|| held.get()));
                let stored = create_memo(move || {
                    Some(entities.followed(held.get()?, |signals| signals.held))
                });
                let shown = create_memo(move || {
                    Some(entities.followed(held_shown.get()?, |signals| signals.held_shown))
                });
                let frame = create_memo(move || {
                    held_shown.with(|entity| entity.as_ref().is_some_and(CanvasEntity::is_frame))
                });
                let embeds = create_memo(move || {
                    held_shown.with(|entity| {
                        entity.as_ref().is_some_and(|entity| {
                            entity.block.is_some() && entity.shape.embeds_block()
                        })
                    })
                });
                (
                    EntitySignals {
                        held,
                        held_shown,
                        stored,
                        shown,
                        frame,
                        embeds,
                    },
                    preview,
                )
            })
        });
        if !slots.catalog.borrow().listed.contains_key(&id) {
            slots.released.borrow_mut().insert(id);
        }
        slots.slots.borrow_mut().insert(
            id,
            Slot {
                signals,
                preview,
                watchers: 0,
                _scope: scope,
            },
        );
        signals
    }

    fn unwatch(&self, id: Uuid) {
        let Some(slots) = self.slots.try_value() else {
            return;
        };
        if let Some(slot) = slots.slots.borrow_mut().get_mut(&id) {
            slot.watchers = slot.watchers.saturating_sub(1);
        }
        self.release(&slots, &[id]);
    }

    fn sweep(&self) {
        let Some(slots) = self.slots.try_value() else {
            return;
        };
        let released: Vec<Uuid> = slots.released.borrow_mut().drain().collect();
        self.release(&slots, &released);
    }

    fn release(&self, slots: &Slots, ids: &[Uuid]) {
        let held = untrack(|| {
            slots.content.read(|content| {
                ids.iter()
                    .filter(|id| content.tree().contains(ObjectId::from_uuid(**id)))
                    .copied()
                    .collect::<HashSet<_>>()
            })
        })
        .unwrap_or_default();
        let previewed = slots.previewed.borrow();
        let released: Vec<Slot> = {
            let mut map = slots.slots.borrow_mut();
            let idle: Vec<Uuid> = ids
                .iter()
                .filter(|id| !held.contains(id) && !previewed.contains(id))
                .filter(|id| map.get(id).is_some_and(|slot| slot.watchers == 0))
                .copied()
                .collect();
            idle.iter().filter_map(|id| map.remove(id)).collect()
        };
        drop(previewed);
        drop(released);
    }
}
