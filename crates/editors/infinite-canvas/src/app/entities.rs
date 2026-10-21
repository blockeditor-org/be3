use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use block_editor_beui::ContentProjection;
use block_editor_beui::be_block::CanvasContent;
use block_editor_beui::be_block::ObjectId;
use block_editor_beui::be_block::canvas::{Canvas, CanvasEntity, Entity};
use block_editor_beui::beui::reactive::*;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) struct EntitySignals {
    held: Memo<Option<CanvasEntity>>,
    held_shown: Memo<Option<CanvasEntity>>,
    stored: Memo<Option<CanvasEntity>>,
    shown: Memo<Option<CanvasEntity>>,
    frame: Memo<bool>,
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
    previewed: RefCell<HashSet<Uuid>>,
}

#[derive(Clone, Copy)]
pub(crate) struct Entities {
    slots: Stored<Slots>,
    ids: Memo<Vec<Uuid>>,
    previewed: Signal<Vec<Uuid>>,
    shown_ids: Memo<Vec<Uuid>>,
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
            previewed: RefCell::new(HashSet::new()),
        });
        let entities = Self {
            slots,
            ids,
            previewed,
            shown_ids,
        };
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
        if let Some(slot) = self.slots.value().slots.borrow_mut().get_mut(&id) {
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

    pub(crate) fn first_frame(&self) -> Option<CanvasEntity> {
        self.ids.with(|ids| {
            ids.iter()
                .map(|id| self.signals(*id))
                .find(|signals| signals.frame.get())
                .and_then(|signals| signals.stored.get())
        })
    }

    pub(crate) fn shown_frames(&self) -> Vec<CanvasEntity> {
        self.shown_ids.with(|ids| {
            ids.iter()
                .map(|id| self.signals(*id))
                .filter(|signals| signals.frame.get())
                .filter_map(|signals| signals.shown.get())
                .collect()
        })
    }

    fn followed(
        &self,
        mut entity: CanvasEntity,
        target: fn(EntitySignals) -> Memo<Option<CanvasEntity>>,
    ) -> CanvasEntity {
        if entity.bindings.iter().any(Option::is_some) {
            entity.follow(|id| target(self.pin(id)).get());
        }
        entity
    }

    fn pin(&self, id: Uuid) -> EntitySignals {
        let signals = self.signals(id);
        if let Some(slot) = self.slots.value().slots.borrow_mut().get_mut(&id) {
            slot.watchers += 1;
        }
        let entities = *self;
        on_cleanup(move || entities.unwatch(id));
        signals
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
                (
                    EntitySignals {
                        held,
                        held_shown,
                        stored,
                        shown,
                        frame,
                    },
                    preview,
                )
            })
        });
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
        let idle: Vec<Uuid> = slots
            .slots
            .borrow()
            .iter()
            .filter(|(_, slot)| slot.watchers == 0)
            .map(|(id, _)| *id)
            .collect();
        self.release(&slots, &idle);
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
