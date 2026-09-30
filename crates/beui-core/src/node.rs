use std::any::Any;
use std::cell::{Cell, RefCell};

use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{Modifiers, SecondaryDrag};
use crate::painter::Painter;

use crate::document::Document;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId {
    index: u32,
    generation: u32,
}

impl NodeId {
    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone, Copy)]
pub struct InteractInput {
    pub pointer_pos: Option<Pos2>,
    pub pointer_down: bool,
    pub pressed_this_frame: bool,
    pub released_this_frame: bool,
    pub secondary_pressed_this_frame: bool,
    pub secondary_drag: Option<SecondaryDrag>,
    pub middle_down: bool,
    pub middle_pressed_this_frame: bool,
    pub middle_released_this_frame: bool,
    pub scroll: Vec2,
    pub scroll_fling: Vec2,
    pub zoom: f32,
    pub touch_pan: Vec2,
    pub zoom_pos: Option<Pos2>,
    pub wheel_target: Option<NodeId>,
    pub zoom_target: Option<NodeId>,
    pub touch_started: bool,
    pub touch_active: bool,
    pub touch_ended: bool,
    pub touch_cancelled: bool,
    pub touch_dragged: bool,
    pub touch_scrolling: bool,
    pub touch_scroll_delta: Vec2,
    pub touch_velocity: Vec2,
    pub touch_scroll_target: Option<NodeId>,
    pub clicks: u32,
    pub modifiers: Modifiers,
    pub visible: Rect,
}

impl InteractInput {
    pub fn over(&self, rect: Rect, pos: Pos2) -> bool {
        rect.contains_half_open(pos) && self.visible.contains_half_open(pos)
    }

    pub fn pointer_over(&self, rect: Rect) -> bool {
        self.pointer_pos.is_some_and(|pos| self.over(rect, pos))
    }
}

pub type Handler<V, R = ()> = Box<dyn FnMut(V) -> R>;
pub type ClickHandler = Box<dyn FnMut()>;

pub trait Element: Any {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2;

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects);

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect);

    fn unplaced(&mut self, _doc: &mut Document) {}

    fn paints(&self) -> bool {
        true
    }

    fn relayout_boundary(&self) -> bool {
        false
    }

    fn tracks_stale_children(&self) -> bool {
        false
    }

    fn engaged(&self) -> bool {
        false
    }

    fn captures(&mut self, _doc: &mut Document, _pos: Pos2, _rect: Rect) -> bool {
        false
    }

    fn intercepts(&mut self, _doc: &mut Document, _pos: Pos2, _rect: Rect) -> bool {
        false
    }

    fn interact(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    );

    fn children(&self) -> Vec<NodeId>;

    fn live_children(&self) -> Vec<NodeId> {
        self.children()
    }

    fn borrowed(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn kind(&self) -> &'static str;

    fn detail(&self) -> Option<String> {
        None
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
        match self.entries.get(id.index as usize)? {
            Some((generation, value)) if *generation == id.generation => Some(value),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, id: &NodeId) -> Option<&mut T> {
        match self.entries.get_mut(id.index as usize)? {
            Some((generation, value)) if *generation == id.generation => Some(value),
            _ => None,
        }
    }

    pub fn contains_key(&self, id: &NodeId) -> bool {
        self.get(id).is_some()
    }

    pub fn insert(&mut self, id: NodeId, value: T) -> Option<T> {
        let slot = self.slot(id);
        match slot {
            Some((generation, _)) if *generation > id.generation => None,
            _ => slot
                .replace((id.generation, value))
                .filter(|(generation, _)| *generation == id.generation)
                .map(|(_, value)| value),
        }
    }

    pub fn remove(&mut self, id: &NodeId) -> Option<T> {
        let slot = self.entries.get_mut(id.index as usize)?;
        match slot {
            Some((generation, _)) if *generation == id.generation => {
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
                entry.as_ref().map(|(generation, value)| {
                    (
                        NodeId {
                            index: index as u32,
                            generation: *generation,
                        },
                        value,
                    )
                })
            })
    }

    fn slot(&mut self, id: NodeId) -> &mut Option<(u32, T)> {
        let index = id.index as usize;
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
            .is_none_or(|(generation, _)| *generation != id.generation)
        {
            *slot = Some((id.generation, T::default()));
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

    pub fn inside(node: NodeId, slot: u8) -> Self {
        assert!(
            (1..SPACE_SLOTS as u8).contains(&slot),
            "a node has {} spaces inside it",
            SPACE_SLOTS - 1
        );
        Self { node, slot }
    }
}

const SPACE_SLOTS: usize = 3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Placed {
    pub rect: Rect,
    pub space: Option<SpaceId>,
}

#[derive(Clone, Copy, PartialEq)]
struct Space {
    parent: Option<SpaceId>,
    translation: Vec2,
    clip: Rect,
}

type Resolved = (u64, Vec2);

type ResolvedClip = (u64, Rect);

#[derive(Default)]
pub struct Rects {
    map: RefCell<NodeMap<Placed>>,
    spaces: RefCell<NodeMap<[Option<Space>; SPACE_SLOTS]>>,
    offsets: RefCell<NodeMap<[Option<Resolved>; SPACE_SLOTS]>>,
    clips: RefCell<NodeMap<[Option<ResolvedClip>; SPACE_SLOTS]>>,
    version: Cell<u64>,
    moves: Cell<u64>,
}

impl Rects {
    pub fn get(&self, id: &NodeId) -> Option<Rect> {
        let placed = self.placed(id)?;
        Some(placed.rect.translate(self.offset(placed.space)))
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
        let space = Space {
            parent,
            translation,
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

    fn space(&self, id: SpaceId) -> Option<Space> {
        self.spaces
            .borrow()
            .get(&id.node)
            .and_then(|slots| slots[id.slot as usize])
    }

    pub fn offset(&self, space: Option<SpaceId>) -> Vec2 {
        let stamp = self.moves.get();
        let mut chain = Vec::new();
        let mut current = space;
        let mut offset = Vec2::ZERO;
        while let Some(id) = current {
            let held = self
                .offsets
                .borrow()
                .get(&id.node)
                .and_then(|slots| slots[id.slot as usize]);
            if let Some((held_stamp, resolved)) = held
                && held_stamp == stamp
            {
                offset = resolved;
                break;
            }
            let Some(space) = self.space(id) else {
                break;
            };
            if chain.len() >= ANCESTOR_LIMIT {
                break;
            }
            chain.push((id, space.translation));
            current = space.parent;
        }
        let mut offsets = self.offsets.borrow_mut();
        for (id, translation) in chain.into_iter().rev() {
            offset += translation;
            offsets.get_or_default(id.node)[id.slot as usize] = Some((stamp, offset));
        }
        offset
    }

    pub fn space_clip(&self, space: Option<SpaceId>) -> Rect {
        let stamp = self.moves.get();
        let mut chain = Vec::new();
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
        for (id, held) in chain.into_iter().rev() {
            clip = clip.intersect(held.clip).translate(-held.translation);
            clips.get_or_default(id.node)[id.slot as usize] = Some((stamp, clip));
        }
        clip
    }

    pub fn visible(&self, id: &NodeId) -> Option<Rect> {
        let rect = self.get(id)?;
        let own = Some(SpaceId::of(*id));
        let clip = self.space_clip(own).translate(self.offset(own));
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

const STALE_CHILDREN_LIMIT: usize = 1024;

#[derive(Default)]
pub struct StaleChildren {
    pub nodes: Vec<NodeId>,
    pub overflowed: bool,
}

#[derive(Default)]
pub struct Arena {
    nodes: Vec<Option<Box<dyn Element>>>,
    generations: Vec<u32>,
    parents: Vec<Option<NodeId>>,
    stale: Vec<bool>,
    unplaced: Vec<bool>,
    tracks: Vec<bool>,
    stale_children: NodeMap<StaleChildren>,
    free: Vec<u32>,
    released: Vec<u32>,
    boundaries: Vec<NodeId>,
    marked: Option<NodeId>,
    live: usize,
    pub revision: u64,
    pub layout_revision: u64,
    pub epoch: u64,
    changed: Vec<NodeId>,
    relaid: Vec<NodeId>,
    repaints: Vec<NodeId>,
    everything: bool,
}

impl Arena {
    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    pub fn insert<T: Element>(&mut self, element: T) -> NodeId {
        let tracks = element.tracks_stale_children();
        let id = match self.free.pop() {
            Some(index) => {
                let slot = index as usize;
                self.nodes[slot] = Some(Box::new(element));
                self.parents[slot] = None;
                self.stale[slot] = true;
                self.unplaced[slot] = true;
                self.tracks[slot] = tracks;
                NodeId {
                    index,
                    generation: self.generations[slot],
                }
            }
            None => {
                let index = self.nodes.len() as u32;
                self.nodes.push(Some(Box::new(element)));
                self.generations.push(0);
                self.parents.push(None);
                self.stale.push(true);
                self.unplaced.push(true);
                self.tracks.push(tracks);
                NodeId {
                    index,
                    generation: 0,
                }
            }
        };
        self.live += 1;
        self.invalidate_node(id);
        id
    }

    fn slot(&self, id: NodeId) -> Option<usize> {
        let index = id.index as usize;
        (self.generations.get(index) == Some(&id.generation)).then_some(index)
    }

    pub fn current(&self, id: NodeId) -> bool {
        self.slot(id).is_some()
    }

    pub fn id_at(&self, index: u32) -> Option<NodeId> {
        let generation = *self.generations.get(index as usize)?;
        Some(NodeId { index, generation })
    }

    pub fn contains(&self, id: NodeId) -> bool {
        self.slot(id).is_some_and(|slot| self.nodes[slot].is_some())
    }

    fn element(&self, id: NodeId) -> Option<&dyn Element> {
        self.nodes[self.slot(id)?].as_deref()
    }

    fn element_mut(&mut self, id: NodeId) -> &mut dyn Element {
        let slot = self.slot(id).expect("node was removed");
        self.nodes[slot].as_deref_mut().expect("node was removed")
    }

    pub fn get(&self, id: NodeId) -> &dyn Element {
        self.element(id).expect("node was removed")
    }

    pub fn get_mut(&mut self, id: NodeId) -> &mut dyn Element {
        self.invalidate_node(id);
        self.element_mut(id)
    }

    pub fn paint_mut_as<T: Element>(&mut self, id: NodeId) -> &mut T {
        self.repaint_node(id);
        self.downcast_mut(id)
    }

    pub fn touch_mut_as<T: Element>(&mut self, id: NodeId) -> &mut T {
        self.changed.push(id);
        self.downcast_mut(id)
    }

    pub fn touch_mut(&mut self, id: NodeId) -> &mut dyn Element {
        self.changed.push(id);
        self.element_mut(id)
    }

    fn downcast_mut<T: Element>(&mut self, id: NodeId) -> &mut T {
        self.element_mut(id)
            .as_any_mut()
            .downcast_mut::<T>()
            .unwrap_or_else(|| panic!("node is not a {}", std::any::type_name::<T>()))
    }

    pub fn get_as<T: Element>(&self, id: NodeId) -> &T {
        self.get(id)
            .as_any()
            .downcast_ref::<T>()
            .unwrap_or_else(|| panic!("node is not a {}", std::any::type_name::<T>()))
    }

    pub fn get_mut_as<T: Element>(&mut self, id: NodeId) -> &mut T {
        self.get_mut(id)
            .as_any_mut()
            .downcast_mut::<T>()
            .unwrap_or_else(|| panic!("node is not a {}", std::any::type_name::<T>()))
    }

    pub fn take(&mut self, id: NodeId) -> Box<dyn Element> {
        let slot = self.slot(id).expect("node was removed");
        self.nodes[slot].take().expect("node was removed")
    }

    pub fn put_back(&mut self, id: NodeId, element: Box<dyn Element>) {
        let slot = self.slot(id).expect("node was removed");
        self.nodes[slot] = Some(element);
    }

    pub fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.epoch = self.epoch.wrapping_add(1);
        self.everything = true;
        self.stale_children = NodeMap::default();
        self.stale.fill(true);
        self.unplaced.fill(true);
        self.boundaries.clear();
        self.marked = None;
    }

    pub fn invalidate_node(&mut self, id: NodeId) {
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.relaid.push(id);
        self.repaint_node(id);
        self.mark_stale(id);
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
        if self.marked == Some(id) {
            return;
        }
        self.marked = Some(id);
        let mut current = Some(id);
        let mut child = None;
        for _ in 0..ANCESTOR_LIMIT {
            let Some((node, slot)) = current.and_then(|node| Some((node, self.slot(node)?))) else {
                return;
            };
            if let Some(child) = child
                && self.tracks[slot]
            {
                self.note_stale_child(node, child);
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
            self.stale[slot] = true;
            self.unplaced[slot] = true;
            child = Some(node);
            current = self.parents[slot];
        }
    }

    fn note_stale_child(&mut self, parent: NodeId, child: NodeId) {
        let held = self.stale_children.get_or_default(parent);
        if held.overflowed || held.nodes.last() == Some(&child) {
            return;
        }
        if held.nodes.len() >= STALE_CHILDREN_LIMIT {
            held.nodes = Vec::new();
            held.overflowed = true;
            return;
        }
        held.nodes.push(child);
    }

    pub fn take_stale_children(&mut self, id: NodeId) -> StaleChildren {
        self.stale_children.remove(&id).unwrap_or_default()
    }

    pub fn take_boundaries(&mut self) -> Vec<NodeId> {
        let mut boundaries = std::mem::take(&mut self.boundaries);
        boundaries.sort_unstable_by_key(|id| (id.index, id.generation));
        boundaries.dedup();
        boundaries
    }

    pub fn stale(&self, id: NodeId) -> bool {
        self.slot(id).is_none_or(|slot| self.stale[slot])
    }

    pub fn clear_stale(&mut self, id: NodeId) {
        if let Some(slot) = self.slot(id) {
            self.stale[slot] = false;
        }
        self.marked = None;
    }

    pub fn unplaced(&self, id: NodeId) -> bool {
        self.slot(id).is_none_or(|slot| self.unplaced[slot])
    }

    pub fn clear_unplaced(&mut self, id: NodeId) {
        if let Some(slot) = self.slot(id) {
            self.unplaced[slot] = false;
        }
        self.marked = None;
    }

    pub fn set_parent(&mut self, id: NodeId, parent: Option<NodeId>) {
        let Some(slot) = self.slot(id) else {
            return;
        };
        if self.parents[slot] == parent {
            return;
        }
        self.parents[slot] = parent;
        self.marked = None;
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.parents[self.slot(id)?].filter(|parent| self.current(*parent))
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
        changed.sort_unstable_by_key(|id| (id.index, id.generation));
        changed.dedup();
        changed
    }

    pub fn take_everything(&mut self) -> bool {
        std::mem::take(&mut self.everything)
    }

    pub fn remove(&mut self, id: NodeId) {
        self.invalidate_node(id);
        let Some(slot) = self.slot(id) else {
            return;
        };
        self.stale_children.remove(&id);
        if self.nodes[slot].take().is_some() {
            self.live -= 1;
            self.released.push(id.index);
        }
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
