use std::any::Any;
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;

use crate::painter::Painter;
use beui_types::PixelGrid;
use beui_types::layout::Direction;
use beui_types::{Pos2, Rect, Vec2, pos2};

use crate::document::Document;

use beui_types::NodeId;

#[derive(Default)]
struct Removals(RefCell<Vec<NodeId>>);

impl ::reactive::Release for Removals {
    fn release(&self, key: u64) {
        self.0.borrow_mut().push(NodeId::from_key(key));
    }
}

pub struct NodeOf<T> {
    id: NodeId,
    kind: PhantomData<fn() -> T>,
}

impl<T> NodeOf<T> {
    pub fn id(self) -> NodeId {
        self.id
    }

    pub(crate) fn assumed(id: NodeId) -> Self {
        Self {
            id,
            kind: PhantomData,
        }
    }
}

impl<T> Clone for NodeOf<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for NodeOf<T> {}

impl<T> PartialEq for NodeOf<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for NodeOf<T> {}

impl<T> std::hash::Hash for NodeOf<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T> std::fmt::Debug for NodeOf<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.id.fmt(f)
    }
}

impl<T> From<NodeOf<T>> for NodeId {
    fn from(node: NodeOf<T>) -> Self {
        node.id
    }
}

pub type Handler<V, R = ()> = Box<dyn FnMut(V) -> R>;
pub type ClickHandler = Box<dyn FnMut()>;

pub enum FocusChild {
    Node(NodeId),
    Unbuilt(std::ops::Range<usize>),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Outlined {
    pub inner: Rect,
    pub outer: Rect,
}

pub trait Element: Any {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2;

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects);

    fn baseline(&self, _doc: &mut Document, _painter: &Painter, _available: Vec2) -> Option<f32> {
        None
    }

    fn min_width(&self, _doc: &mut Document, _painter: &Painter) -> f32 {
        0.0
    }

    fn least_length(
        &self,
        _doc: &mut Document,
        _painter: &Painter,
        _direction: Direction,
        _cross: f32,
    ) -> Option<f32> {
        None
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect);

    fn unplaced(&mut self, _doc: &mut Document) {}

    fn detached(&mut self) {}

    fn paints(&self) -> bool {
        true
    }

    fn relayout_boundary(&self) -> bool {
        false
    }

    fn paints_from_layout(&self) -> bool {
        true
    }

    fn passes_scroll_anchor(&self) -> bool {
        false
    }

    fn turns_children(&self) -> bool {
        false
    }

    fn children(&self) -> Vec<NodeId>;

    fn for_each_child(&self, visit: &mut dyn FnMut(NodeId)) {
        for child in self.children() {
            visit(child);
        }
    }

    fn for_each_shown_child(&self, visit: &mut dyn FnMut(NodeId)) {
        self.for_each_child(visit);
    }

    fn children_revision(&self) -> Option<u64> {
        None
    }

    fn remove_child(&mut self, child: NodeId);

    fn focus_children(&self) -> Option<Vec<FocusChild>> {
        None
    }

    fn build_for_focus(
        &mut self,
        _doc: &mut Document,
        _id: NodeId,
        _index: usize,
    ) -> Option<NodeId> {
        None
    }

    fn release_for_focus(&mut self, _child: NodeId) {}

    fn live_children(&self) -> Vec<NodeId> {
        self.children()
    }

    fn pointer_children(&self) -> Vec<NodeId> {
        self.children()
    }

    fn borrowed(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn kind(&self) -> &'static str;

    fn ink(&self) -> Option<Rect> {
        None
    }

    fn outline(&self, _grid: PixelGrid, _rect: Rect) -> Option<Outlined> {
        None
    }

    fn detail(&self) -> Option<String> {
        None
    }

    fn properties(&self) -> Vec<(&'static str, String)> {
        Vec::new()
    }

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Clone)]
pub struct NodeMap<T> {
    entries: Vec<Option<(u32, T)>>,
}

impl<T> Default for NodeMap<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T> NodeMap<T> {
    pub fn get(&self, id: &NodeId) -> Option<&T> {
        match self.entries.get(id.index() as usize)? {
            Some((generation, value)) if *generation == id.generation() => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: &NodeId) -> Option<&mut T> {
        match self.entries.get_mut(id.index() as usize)? {
            Some((generation, value)) if *generation == id.generation() => Some(value),
            _ => None,
        }
    }

    pub fn contains_key(&self, id: &NodeId) -> bool {
        self.get(id).is_some()
    }

    pub fn insert(&mut self, id: NodeId, value: T) -> Option<T> {
        let slot = self.slot(id);
        match slot {
            Some((generation, _)) if *generation > id.generation() => None,
            _ => slot
                .replace((id.generation(), value))
                .filter(|(generation, _)| *generation == id.generation())
                .map(|(_, value)| value),
        }
    }

    pub fn clear(&mut self) {
        self.entries.fill_with(|| None);
    }

    pub fn remove(&mut self, id: &NodeId) -> Option<T> {
        let slot = self.entries.get_mut(id.index() as usize)?;
        match slot {
            Some((generation, _)) if *generation == id.generation() => {
                slot.take().map(|(_, value)| value)
            }
            _ => None,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                entry
                    .as_ref()
                    .map(|(generation, value)| (NodeId::new(index as u32, *generation), value))
            })
    }

    fn slot(&mut self, id: NodeId) -> &mut Option<(u32, T)> {
        let index = id.index() as usize;
        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, || None);
        }
        &mut self.entries[index]
    }
}

impl<T: Default> NodeMap<T> {
    pub fn get_or_default(&mut self, id: NodeId) -> &mut T {
        let slot = self.slot(id);
        if slot
            .as_ref()
            .is_none_or(|(generation, _)| *generation != id.generation())
        {
            *slot = Some((id.generation(), T::default()));
        }
        &mut slot.as_mut().expect("the entry was just filled").1
    }
}

impl<T> std::ops::Index<&NodeId> for NodeMap<T> {
    type Output = T;

    fn index(&self, id: &NodeId) -> &T {
        self.get(id).expect("node was not placed")
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct SpaceId {
    node: NodeId,
    slot: u8,
}

impl SpaceId {
    pub fn of(node: NodeId) -> Self {
        Self { node, slot: 0 }
    }

    pub fn node(self) -> NodeId {
        self.node
    }

    pub fn slot(self) -> u8 {
        self.slot
    }

    pub fn inside(node: NodeId, slot: u8) -> Self {
        assert!(
            (1..SPACE_SLOTS as u8).contains(&slot),
            "a node has {} spaces inside it",
            SPACE_SLOTS - 1
        );
        Self { node, slot }
    }
}

pub(crate) const SPACE_SLOTS: usize = 4;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SpaceTransform {
    pub offset: Vec2,
    pub scale: f32,
}

impl SpaceTransform {
    pub const IDENTITY: Self = Self {
        offset: Vec2::ZERO,
        scale: 1.0,
    };

    pub fn pos(self, pos: Pos2) -> Pos2 {
        pos2(
            pos.x * self.scale + self.offset.x,
            pos.y * self.scale + self.offset.y,
        )
    }

    pub fn rect(self, rect: Rect) -> Rect {
        match self.scale == 1.0 {
            true => rect.translate(self.offset),
            false => rect.scaled(self.scale).translate(self.offset),
        }
    }

    pub fn local_pos(self, pos: Pos2) -> Pos2 {
        pos2(
            (pos.x - self.offset.x) / self.scale,
            (pos.y - self.offset.y) / self.scale,
        )
    }

    pub fn local_rect(self, rect: Rect) -> Rect {
        match self.scale == 1.0 {
            true => rect.translate(-self.offset),
            false => rect.translate(-self.offset).scaled(self.scale.recip()),
        }
    }

    fn within(self, translation: Vec2, scale: f32) -> Self {
        Self {
            offset: self.offset + translation * self.scale,
            scale: self.scale * scale,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Placed {
    pub rect: Rect,
    pub space: Option<SpaceId>,
}

#[derive(Clone, Copy, PartialEq)]
struct Space {
    parent: Option<SpaceId>,
    translation: Vec2,
    scale: f32,
    clip: Rect,
}

type Resolved = (u64, SpaceTransform);

type ResolvedClip = (u64, Rect);

#[derive(Default)]
pub struct Rects {
    map: RefCell<NodeMap<Placed>>,
    spaces: RefCell<NodeMap<[Option<Space>; SPACE_SLOTS]>>,
    offsets: RefCell<NodeMap<[Option<Resolved>; SPACE_SLOTS]>>,
    clips: RefCell<NodeMap<[Option<ResolvedClip>; SPACE_SLOTS]>>,
    version: Cell<u64>,
    moves: Cell<u64>,
    chain: RefCell<Vec<(SpaceId, Vec2, f32)>>,
    clip_chain: RefCell<Vec<(SpaceId, Space)>>,
}

impl beui_types::drag::Space for Rects {
    fn placed(&self, node: NodeId) -> Option<(Rect, f32)> {
        self.scaled(&node)
    }

    fn visible(&self, node: NodeId) -> Option<Rect> {
        Rects::visible(self, &node)
    }
}

impl Rects {
    pub fn get(&self, id: &NodeId) -> Option<Rect> {
        let placed = self.placed(id)?;
        Some(self.transform(placed.space).rect(placed.rect))
    }

    pub fn scaled(&self, id: &NodeId) -> Option<(Rect, f32)> {
        let placed = self.placed(id)?;
        let transform = self.transform(placed.space);
        Some((transform.rect(placed.rect), transform.scale))
    }

    pub fn local_to_window(&self, id: &NodeId, local: Rect) -> Option<Rect> {
        let placed = self.placed(id)?;
        let transform = self.transform(placed.space);
        Some(transform.rect(local.translate(placed.rect.min.to_vec2())))
    }

    pub fn scale_of(&self, id: &NodeId) -> f32 {
        self.placed(id)
            .map_or(1.0, |placed| self.transform(placed.space).scale)
    }

    pub fn placed(&self, id: &NodeId) -> Option<Placed> {
        self.map.borrow().get(id).copied()
    }

    pub fn contains_key(&self, id: &NodeId) -> bool {
        self.map.borrow().contains_key(id)
    }

    pub fn insert(&self, id: NodeId, placed: Placed) -> Option<Placed> {
        let previous = self.map.borrow_mut().insert(id, placed);
        if previous != Some(placed) {
            self.bump();
        }
        previous
    }

    pub fn remove(&self, id: &NodeId) -> Option<Placed> {
        if self.spaces.borrow_mut().remove(id).is_some() {
            self.moves.set(self.moves.get().wrapping_add(1));
        }
        self.offsets.borrow_mut().remove(id);
        self.clips.borrow_mut().remove(id);
        let removed = self.map.borrow_mut().remove(id);
        if removed.is_some() {
            self.bump();
        }
        removed
    }

    pub fn set_space(
        &self,
        id: SpaceId,
        parent: Option<SpaceId>,
        translation: Vec2,
        clip: Rect,
    ) -> bool {
        self.set_scaled_space(id, parent, translation, 1.0, clip)
    }

    pub fn set_scaled_space(
        &self,
        id: SpaceId,
        parent: Option<SpaceId>,
        translation: Vec2,
        scale: f32,
        clip: Rect,
    ) -> bool {
        let space = Space {
            parent,
            translation,
            scale,
            clip,
        };
        let mut spaces = self.spaces.borrow_mut();
        let held = &mut spaces.get_or_default(id.node)[id.slot as usize];
        let moved = *held != Some(space);
        if moved {
            *held = Some(space);
            self.moves.set(self.moves.get().wrapping_add(1));
            self.bump();
        }
        moved
    }

    pub fn space_of(&self, id: SpaceId) -> Option<(Vec2, f32)> {
        self.space(id).map(|space| (space.translation, space.scale))
    }

    fn space(&self, id: SpaceId) -> Option<Space> {
        self.spaces
            .borrow()
            .get(&id.node)
            .and_then(|slots| slots[id.slot as usize])
    }

    pub fn transform(&self, space: Option<SpaceId>) -> SpaceTransform {
        let stamp = self.moves.get();
        let mut chain = self.chain.borrow_mut();
        let mut current = space;
        let mut transform = SpaceTransform::IDENTITY;
        while let Some(id) = current {
            let held = self
                .offsets
                .borrow()
                .get(&id.node)
                .and_then(|slots| slots[id.slot as usize]);
            if let Some((held_stamp, resolved)) = held
                && held_stamp == stamp
            {
                transform = resolved;
                break;
            }
            let Some(space) = self.space(id) else {
                break;
            };
            if chain.len() >= ANCESTOR_LIMIT {
                break;
            }
            chain.push((id, space.translation, space.scale));
            current = space.parent;
        }
        let mut offsets = self.offsets.borrow_mut();
        for (id, translation, scale) in chain.drain(..).rev() {
            transform = transform.within(translation, scale);
            offsets.get_or_default(id.node)[id.slot as usize] = Some((stamp, transform));
        }
        transform
    }

    pub fn space_clip(&self, space: Option<SpaceId>) -> Rect {
        let stamp = self.moves.get();
        let mut chain = self.clip_chain.borrow_mut();
        let mut current = space;
        let mut clip = Rect::EVERYTHING;
        while let Some(id) = current {
            let held = self
                .clips
                .borrow()
                .get(&id.node)
                .and_then(|slots| slots[id.slot as usize]);
            if let Some((held_stamp, resolved)) = held
                && held_stamp == stamp
            {
                clip = resolved;
                break;
            }
            let Some(space) = self.space(id) else {
                break;
            };
            if chain.len() >= ANCESTOR_LIMIT {
                break;
            }
            chain.push((id, space));
            current = space.parent;
        }
        let mut clips = self.clips.borrow_mut();
        for (id, held) in chain.drain(..).rev() {
            clip = clip.intersect(held.clip).translate(-held.translation);
            if held.scale != 1.0 {
                clip = clip.scaled(held.scale.recip());
            }
            clips.get_or_default(id.node)[id.slot as usize] = Some((stamp, clip));
        }
        clip
    }

    pub fn clips_above(&self, id: &NodeId) -> Vec<(SpaceId, Rect)> {
        let mut clips = Vec::new();
        let mut current = Some(SpaceId::of(*id));
        while let Some(id) = current {
            let Some(space) = self.space(id) else {
                break;
            };
            if clips.len() >= ANCESTOR_LIMIT {
                break;
            }
            clips.push((id, self.transform(space.parent).rect(space.clip)));
            current = space.parent;
        }
        clips
    }

    pub fn visible(&self, id: &NodeId) -> Option<Rect> {
        let rect = self.get(id)?;
        let own = Some(SpaceId::of(*id));
        let clip = self.transform(own).rect(self.space_clip(own));
        Some(rect.intersect(clip))
    }

    fn bump(&self) {
        self.version.set(self.version.get().wrapping_add(1));
    }

    pub fn version(&self) -> u64 {
        self.version.get()
    }
}

pub const ANCESTOR_LIMIT: usize = 4096;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Adoption {
    #[default]
    Settled,
    Queued,
    Taken,
}

#[derive(Default)]
pub struct Arena {
    nodes: Vec<Option<Box<dyn Element>>>,
    generations: Vec<u32>,
    kinds: Vec<std::any::TypeId>,
    parents: RefCell<Vec<Option<NodeId>>>,
    owners: Vec<Option<::reactive::ScopeContext>>,
    removed: Vec<bool>,
    removals: std::rc::Rc<Removals>,
    fence: Option<NodeId>,
    adopting: RefCell<Vec<NodeId>>,
    adoption: RefCell<Vec<Adoption>>,
    adopted: RefCell<Vec<Option<u64>>>,
    stale: Vec<bool>,
    unplaced: Vec<bool>,
    free: Vec<u32>,
    released: Vec<u32>,
    boundaries: Vec<NodeId>,
    mark_epoch: u64,
    marks: Vec<u64>,
    live: usize,
    pub revision: u64,
    pub layout_revision: u64,
    changed: Vec<NodeId>,
    relaid: Vec<NodeId>,
    repaints: Vec<NodeId>,
    everything: bool,
    reparented: Cell<bool>,
    watched: Vec<NodeId>,
    watched_slots: Vec<bool>,
}

impl Arena {
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    pub fn insert<T: Element>(&mut self, element: T) -> NodeOf<T> {
        let id = match self.free.pop() {
            Some(index) => {
                let slot = index as usize;
                self.nodes[slot] = Some(Box::new(element));
                self.kinds[slot] = std::any::TypeId::of::<T>();
                self.parents.get_mut()[slot] = None;
                self.owners[slot] = None;
                self.removed[slot] = false;
                self.stale[slot] = true;
                self.unplaced[slot] = true;
                self.marks[slot] = 0;
                self.adoption.get_mut()[slot] = Adoption::Settled;
                self.adopted.get_mut()[slot] = None;
                NodeId::new(index, self.generations[slot])
            }
            None => {
                let index = self.nodes.len() as u32;
                self.nodes.push(Some(Box::new(element)));
                self.generations.push(0);
                self.kinds.push(std::any::TypeId::of::<T>());
                self.parents.get_mut().push(None);
                self.owners.push(None);
                self.removed.push(false);
                self.stale.push(true);
                self.unplaced.push(true);
                self.marks.push(0);
                self.adoption.get_mut().push(Adoption::Settled);
                self.adopted.get_mut().push(None);
                NodeId::new(index, 0)
            }
        };
        self.live += 1;
        self.invalidate_node(id);
        self.own(id);
        NodeOf {
            id,
            kind: PhantomData,
        }
    }

    fn own(&mut self, id: NodeId) {
        let Some(owner) = ::reactive::current_scope() else {
            return;
        };
        ::reactive::on_release(self.removals.clone(), id.key());
        self.owners[id.index() as usize] = Some(owner);
    }

    pub(crate) fn take_removals(&self) -> Vec<NodeId> {
        std::mem::take(&mut *self.removals.0.borrow_mut())
    }

    pub(crate) fn give_back_removals(&self, mut removals: Vec<NodeId>) {
        removals.clear();
        let mut held = self.removals.0.borrow_mut();
        if held.is_empty() && held.capacity() < removals.capacity() {
            *held = removals;
        }
    }

    pub(crate) fn removals_pending(&self) -> bool {
        !self.removals.0.borrow().is_empty()
    }

    pub fn owner(&self, id: NodeId) -> Option<::reactive::ScopeContext> {
        self.owners[self.slot(id)?].clone()
    }

    pub fn owned(&self, id: NodeId) -> bool {
        self.slot(id).is_some_and(|slot| {
            self.owners[slot]
                .as_ref()
                .is_some_and(|owner| owner.is_alive())
        })
    }

    fn slot(&self, id: NodeId) -> Option<usize> {
        let index = id.index() as usize;
        (self.generations.get(index) == Some(&id.generation())).then_some(index)
    }

    pub fn current(&self, id: NodeId) -> bool {
        self.slot(id).is_some()
    }

    pub fn id_at(&self, index: u32) -> Option<NodeId> {
        let generation = *self.generations.get(index as usize)?;
        Some(NodeId::new(index, generation))
    }

    pub fn contains(&self, id: impl Into<NodeId>) -> bool {
        let id = id.into();
        self.slot(id).is_some_and(|slot| self.nodes[slot].is_some())
    }

    fn element(&self, id: NodeId) -> Option<&dyn Element> {
        self.nodes[self.slot(id)?].as_deref()
    }

    fn element_mut(&mut self, id: NodeId) -> &mut dyn Element {
        let slot = self.slot(id).expect("node was removed");
        self.nodes[slot].as_deref_mut().expect("node was removed")
    }

    pub fn try_get(&self, id: impl Into<NodeId>) -> Option<&dyn Element> {
        self.element(id.into())
    }

    pub fn get(&self, id: impl Into<NodeId>) -> &dyn Element {
        let id = id.into();
        self.element(id).expect("node was removed")
    }

    pub fn get_mut(&mut self, id: impl Into<NodeId>) -> &mut dyn Element {
        let id = id.into();
        self.invalidate_node(id);
        self.element_mut(id)
    }

    pub fn state_mut(&mut self, id: impl Into<NodeId>) -> &mut dyn Element {
        self.element_mut(id.into())
    }

    pub fn paint_mut_as<T: Element>(&mut self, node: NodeOf<T>) -> &mut T {
        self.repaint_node(node.id);
        self.downcast_mut(node)
    }

    pub fn touch_mut_as<T: Element>(&mut self, node: NodeOf<T>) -> &mut T {
        self.changed.push(node.id);
        self.downcast_mut(node)
    }

    pub fn touch(&mut self, id: NodeId) {
        self.changed.push(id);
    }

    pub fn touch_mut(&mut self, id: NodeId) -> &mut dyn Element {
        self.changed.push(id);
        self.element_mut(id)
    }

    fn downcast_mut<T: Element>(&mut self, node: NodeOf<T>) -> &mut T {
        self.element_mut(node.id)
            .as_any_mut()
            .downcast_mut::<T>()
            .expect("a node keeps the kind it was made as")
    }

    pub fn get_as<T: Element>(&self, node: NodeOf<T>) -> &T {
        self.get(node.id)
            .as_any()
            .downcast_ref::<T>()
            .expect("a node keeps the kind it was made as")
    }

    pub fn get_mut_as<T: Element>(&mut self, node: NodeOf<T>) -> &mut T {
        self.get_mut(node.id)
            .as_any_mut()
            .downcast_mut::<T>()
            .expect("a node keeps the kind it was made as")
    }

    pub fn made_as<T: Element>(&self, id: NodeId) -> Option<NodeOf<T>> {
        let slot = self.slot(id)?;
        (self.kinds[slot] == std::any::TypeId::of::<T>()).then_some(NodeOf {
            id,
            kind: PhantomData,
        })
    }

    pub fn taken(&self, id: NodeId) -> bool {
        self.slot(id).is_some_and(|slot| self.nodes[slot].is_none())
    }

    pub fn kind_of<T: Element>(&self, id: NodeId) -> Option<NodeOf<T>> {
        self.element(id)?.as_any().is::<T>().then_some(NodeOf {
            id,
            kind: PhantomData,
        })
    }

    pub fn take(&mut self, id: impl Into<NodeId>) -> Box<dyn Element> {
        let id = id.into();
        let slot = self.slot(id).expect("node was removed");
        self.nodes[slot].take().expect("node was removed")
    }

    pub fn put_back(&mut self, id: impl Into<NodeId>, element: Box<dyn Element>) {
        let id = id.into();
        let slot = self.slot(id).expect("node was removed");
        self.nodes[slot] = Some(element);
        if self.adoption.get_mut()[slot] == Adoption::Taken {
            self.queue_adoption(id);
        }
    }

    pub fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.everything = true;
        self.stale.fill(true);
        self.unplaced.fill(true);
        self.boundaries.clear();
        self.forget_marks();
    }

    pub fn invalidate_node(&mut self, id: impl Into<NodeId>) {
        let id = id.into();
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.relaid.push(id);
        self.repaint_node(id);
        self.mark_stale(id);
        self.queue_adoption(id);
    }

    pub fn relayout_alone_mut_as<T: Element>(&mut self, node: NodeOf<T>) -> &mut T {
        let id = node.id;
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.relaid.push(id);
        self.repaint_node(id);
        if let Some(slot) = self.slot(id) {
            self.stale[slot] = true;
            self.unplaced[slot] = true;
            self.boundaries.push(id);
        }
        self.forget_marks();
        self.downcast_mut(node)
    }

    fn queue_adoption(&mut self, id: NodeId) {
        let Some(slot) = self.slot(id) else {
            return;
        };
        let adoption = &mut self.adoption.get_mut()[slot];
        if *adoption != Adoption::Queued {
            *adoption = Adoption::Queued;
            self.adopting.get_mut().push(id);
        }
    }

    pub fn repaint_node(&mut self, id: NodeId) {
        self.revision = self.revision.wrapping_add(1);
        self.changed.push(id);
        self.repaints.push(id);
    }

    pub fn note_relaid(&mut self, id: NodeId) {
        self.repaints.push(id);
    }

    pub fn take_repaints(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.repaints)
    }

    fn mark_stale(&mut self, id: NodeId) {
        if self.adopt() {
            self.forget_marks();
        }
        let epoch = self.mark_epoch.wrapping_add(1);
        let mut current = Some(id);
        for _ in 0..ANCESTOR_LIMIT {
            let Some((node, slot)) = current.and_then(|node| Some((node, self.slot(node)?))) else {
                return;
            };
            if self.fence == Some(node) {
                return;
            }
            if node != id
                && self.nodes[slot]
                    .as_deref()
                    .is_some_and(Element::relayout_boundary)
            {
                self.unplaced[slot] = true;
                self.boundaries.push(node);
                return;
            }
            if self.marks[slot] == epoch {
                return;
            }
            self.marks[slot] = epoch;
            self.stale[slot] = true;
            self.unplaced[slot] = true;
            current = self.parents.get_mut()[slot];
        }
    }

    fn forget_marks(&mut self) {
        self.mark_epoch = self.mark_epoch.wrapping_add(1);
    }

    pub(crate) fn fence(&mut self, fence: Option<NodeId>) -> Option<NodeId> {
        self.forget_marks();
        std::mem::replace(&mut self.fence, fence)
    }

    pub fn take_boundaries(&mut self) -> Vec<NodeId> {
        let mut boundaries = std::mem::take(&mut self.boundaries);
        boundaries.sort_unstable_by_key(|id| (id.index(), id.generation()));
        boundaries.dedup();
        boundaries.sort_by_cached_key(|id| self.depth(*id));
        boundaries
    }

    fn depth(&self, id: NodeId) -> usize {
        let mut depth = 0;
        let mut current = self.parent(id);
        while let Some(parent) = current
            && depth < ANCESTOR_LIMIT
        {
            depth += 1;
            current = self.parent(parent);
        }
        depth
    }

    pub fn stale(&self, id: NodeId) -> bool {
        self.slot(id).is_none_or(|slot| self.stale[slot])
    }

    pub fn clear_stale(&mut self, id: NodeId) {
        if let Some(slot) = self.slot(id) {
            self.stale[slot] = false;
        }
        self.forget_marks();
    }

    pub fn unplaced(&self, id: NodeId) -> bool {
        self.slot(id).is_none_or(|slot| self.unplaced[slot])
    }

    pub fn clear_unplaced(&mut self, id: NodeId) {
        if let Some(slot) = self.slot(id) {
            self.unplaced[slot] = false;
        }
        self.forget_marks();
    }

    fn adopt(&self) -> bool {
        let mut pending = std::mem::take(&mut *self.adopting.borrow_mut());
        if pending.is_empty() {
            *self.adopting.borrow_mut() = pending;
            return false;
        }
        let mut moved = false;
        let mut parents = self.parents.borrow_mut();
        let mut adoption = self.adoption.borrow_mut();
        let mut adopted = self.adopted.borrow_mut();
        for parent in pending.drain(..) {
            let Some(slot) = self.slot(parent) else {
                continue;
            };
            if adoption[slot] != Adoption::Queued {
                continue;
            }
            adoption[slot] = Adoption::Settled;
            if self.watched_slots.get(slot).copied().unwrap_or(false) {
                self.reparented.set(true);
            }
            if self.removed[slot] {
                continue;
            }
            let Some(element) = self.nodes[slot].as_deref() else {
                adoption[slot] = Adoption::Taken;
                continue;
            };
            let revision = element.children_revision();
            if revision.is_some() && adopted[slot] == revision {
                continue;
            }
            adopted[slot] = revision;
            element.for_each_child(&mut |child| {
                let Some(held) = self.slot(child).map(|slot| &mut parents[slot]) else {
                    return;
                };
                if *held != Some(parent) {
                    *held = Some(parent);
                    moved = true;
                }
            });
        }
        drop(parents);
        drop(adopted);
        let mut adopting = self.adopting.borrow_mut();
        if adopting.is_empty() {
            *adopting = pending;
        }
        if moved {
            self.reparented.set(true);
        }
        moved
    }

    pub fn adopt_children(&mut self, id: NodeId) {
        self.queue_adoption(id);
    }

    pub fn watch(&mut self, path: Vec<NodeId>) {
        for id in std::mem::take(&mut self.watched) {
            if let Some(watched) = self.watched_slots.get_mut(id.index() as usize) {
                *watched = false;
            }
        }
        self.watched_slots.resize(self.nodes.len(), false);
        for id in &path {
            if let Some(slot) = self.slot(*id) {
                self.watched_slots[slot] = true;
            }
        }
        self.watched = path;
    }

    pub fn take_reparented(&mut self) -> bool {
        self.adopt();
        self.reparented.replace(false)
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.adopt();
        self.parents.borrow()[self.slot(id)?]
            .filter(|parent| self.slot(*parent).is_some_and(|slot| !self.removed[slot]))
    }

    pub fn relaid_len(&self) -> usize {
        self.relaid.len()
    }

    pub fn relaid_since(&self, watermark: usize) -> &[NodeId] {
        self.relaid.get(watermark..).unwrap_or_default()
    }

    pub fn changed(&self) -> &[NodeId] {
        &self.changed
    }

    pub fn take_changed(&mut self) -> Vec<NodeId> {
        self.relaid.clear();
        let mut changed = std::mem::take(&mut self.changed);
        changed.sort_unstable_by_key(|id| (id.index(), id.generation()));
        changed.dedup();
        changed
    }

    pub fn take_everything(&mut self) -> bool {
        std::mem::take(&mut self.everything)
    }

    pub fn remove(&mut self, id: NodeId) -> Option<Box<dyn Element>> {
        self.invalidate_node(id);
        let slot = self.slot(id)?;
        self.owners[slot] = None;
        let element = self.nodes[slot].take()?;
        self.removed[slot] = true;
        self.live -= 1;
        self.released.push(id.index());
        Some(element)
    }

    pub fn take_released(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.released)
    }

    pub fn recycle(&mut self, released: Vec<u32>) {
        for index in released {
            let slot = index as usize;
            if self.nodes[slot].is_some() {
                continue;
            }
            self.generations[slot] = self.generations[slot].wrapping_add(1);
            self.free.push(index);
        }
    }
}
