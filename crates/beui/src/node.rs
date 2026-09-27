use std::any::Any;
use std::cell::RefCell;

use crate::geometry::{Pos2, Rect, Vec2};
use crate::input::{Modifiers, SecondaryDrag};
use crate::painter::Painter;

use crate::document::Document;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NodeId(u32);

impl NodeId {
    pub(crate) fn index(self) -> u32 {
        self.0
    }

    pub(crate) fn from_index(index: u32) -> Self {
        Self(index)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct InteractInput {
    pub(crate) pointer_pos: Option<Pos2>,
    pub(crate) pointer_down: bool,
    pub(crate) pressed_this_frame: bool,
    pub(crate) released_this_frame: bool,
    pub(crate) secondary_pressed_this_frame: bool,
    pub(crate) secondary_drag: Option<SecondaryDrag>,
    pub(crate) middle_down: bool,
    pub(crate) middle_pressed_this_frame: bool,
    pub(crate) scroll: Vec2,
    pub(crate) zoom: f32,
    pub(crate) touch_pan: Vec2,
    pub(crate) zoom_pos: Option<Pos2>,
    pub(crate) wheel_target: Option<NodeId>,
    pub(crate) zoom_target: Option<NodeId>,
    pub(crate) touch_started: bool,
    pub(crate) touch_active: bool,
    pub(crate) touch_ended: bool,
    pub(crate) touch_cancelled: bool,
    pub(crate) touch_dragged: bool,
    pub(crate) touch_scrolling: bool,
    pub(crate) touch_scroll_delta: Vec2,
    pub(crate) touch_velocity: Vec2,
    pub(crate) touch_scroll_target: Option<NodeId>,
    pub(crate) clicks: u32,
    pub(crate) modifiers: Modifiers,
}

pub type Handler<V, R = ()> = Box<dyn FnMut(V) -> R>;
pub type ClickHandler = Box<dyn FnMut()>;

pub(crate) trait Element: Any {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2;

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &Rects,
    );

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect);

    fn unplaced(&mut self, _doc: &mut Document) {}

    fn paints(&self) -> bool {
        true
    }

    fn captures(&mut self, _doc: &mut Document, _pos: Pos2, _rect: Rect) -> bool {
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
    entries: Vec<Option<T>>,
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
        self.entries.get(id.index() as usize)?.as_ref()
    }

    pub fn get_mut(&mut self, id: &NodeId) -> Option<&mut T> {
        self.entries.get_mut(id.index() as usize)?.as_mut()
    }

    pub fn contains_key(&self, id: &NodeId) -> bool {
        self.get(id).is_some()
    }

    pub fn insert(&mut self, id: NodeId, value: T) -> Option<T> {
        let index = id.index() as usize;
        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, || None);
        }
        self.entries[index].replace(value)
    }

    pub fn remove(&mut self, id: &NodeId) -> Option<T> {
        self.entries.get_mut(id.index() as usize)?.take()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (NodeId, &T)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                entry
                    .as_ref()
                    .map(|value| (NodeId::from_index(index as u32), value))
            })
    }
}

impl<T: Default> NodeMap<T> {
    pub fn get_or_default(&mut self, id: NodeId) -> &mut T {
        let index = id.index() as usize;
        if index >= self.entries.len() {
            self.entries.resize_with(index + 1, || None);
        }
        self.entries[index].get_or_insert_with(T::default)
    }
}

impl<T> std::ops::Index<&NodeId> for NodeMap<T> {
    type Output = T;

    fn index(&self, id: &NodeId) -> &T {
        self.get(id).expect("node was not placed")
    }
}

#[derive(Default)]
pub(crate) struct Rects(RefCell<NodeMap<Rect>>);

impl Rects {
    pub(crate) fn get(&self, id: &NodeId) -> Option<Rect> {
        self.0.borrow().get(id).copied()
    }

    pub(crate) fn contains_key(&self, id: &NodeId) -> bool {
        self.0.borrow().contains_key(id)
    }

    pub(crate) fn insert(&self, id: NodeId, rect: Rect) -> Option<Rect> {
        self.0.borrow_mut().insert(id, rect)
    }

    pub(crate) fn remove(&self, id: &NodeId) -> Option<Rect> {
        self.0.borrow_mut().remove(id)
    }
}

pub(crate) const ANCESTOR_LIMIT: usize = 4096;

#[derive(Default)]
pub(crate) struct Arena {
    nodes: Vec<Option<Box<dyn Element>>>,
    parents: Vec<Option<NodeId>>,
    stale: Vec<bool>,
    unplaced: Vec<bool>,
    marked: Option<NodeId>,
    live: usize,
    pub(crate) revision: u64,
    pub(crate) layout_revision: u64,
    changed: Vec<NodeId>,
    relaid: Vec<NodeId>,
    repaints: Vec<NodeId>,
    everything: bool,
}

impl Arena {
    pub(crate) fn len(&self) -> usize {
        self.live
    }

    pub(crate) fn insert<T: Element>(&mut self, element: T) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Some(Box::new(element)));
        self.parents.push(None);
        self.stale.push(true);
        self.unplaced.push(true);
        self.live += 1;
        self.invalidate_node(id);
        id
    }

    pub(crate) fn contains(&self, id: NodeId) -> bool {
        self.nodes
            .get(id.0 as usize)
            .is_some_and(std::option::Option::is_some)
    }

    pub(crate) fn get(&self, id: NodeId) -> &dyn Element {
        self.nodes[id.0 as usize]
            .as_deref()
            .expect("node was removed")
    }

    pub(crate) fn get_mut(&mut self, id: NodeId) -> &mut dyn Element {
        self.invalidate_node(id);
        self.nodes[id.0 as usize]
            .as_deref_mut()
            .expect("node was removed")
    }

    pub(crate) fn paint_mut_as<T: Element>(&mut self, id: NodeId) -> &mut T {
        self.repaint_node(id);
        self.downcast_mut(id)
    }

    pub(crate) fn touch_mut_as<T: Element>(&mut self, id: NodeId) -> &mut T {
        self.changed.push(id);
        self.downcast_mut(id)
    }

    pub(crate) fn touch_mut(&mut self, id: NodeId) -> &mut dyn Element {
        self.changed.push(id);
        self.nodes[id.0 as usize]
            .as_deref_mut()
            .expect("node was removed")
    }

    fn downcast_mut<T: Element>(&mut self, id: NodeId) -> &mut T {
        self.nodes[id.0 as usize]
            .as_deref_mut()
            .expect("node was removed")
            .as_any_mut()
            .downcast_mut::<T>()
            .unwrap_or_else(|| panic!("node is not a {}", std::any::type_name::<T>()))
    }

    pub(crate) fn get_as<T: Element>(&self, id: NodeId) -> &T {
        self.get(id)
            .as_any()
            .downcast_ref::<T>()
            .unwrap_or_else(|| panic!("node is not a {}", std::any::type_name::<T>()))
    }

    pub(crate) fn get_mut_as<T: Element>(&mut self, id: NodeId) -> &mut T {
        self.get_mut(id)
            .as_any_mut()
            .downcast_mut::<T>()
            .unwrap_or_else(|| panic!("node is not a {}", std::any::type_name::<T>()))
    }

    pub(crate) fn take(&mut self, id: NodeId) -> Box<dyn Element> {
        self.nodes[id.0 as usize].take().expect("node was removed")
    }

    pub(crate) fn put_back(&mut self, id: NodeId, element: Box<dyn Element>) {
        self.nodes[id.0 as usize] = Some(element);
    }

    pub(crate) fn invalidate(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.everything = true;
        self.stale.fill(true);
        self.unplaced.fill(true);
        self.marked = None;
    }

    pub(crate) fn invalidate_node(&mut self, id: NodeId) {
        self.layout_revision = self.layout_revision.wrapping_add(1);
        self.relaid.push(id);
        self.repaint_node(id);
        self.mark_stale(id);
    }

    pub(crate) fn repaint_node(&mut self, id: NodeId) {
        self.revision = self.revision.wrapping_add(1);
        self.changed.push(id);
        self.repaints.push(id);
    }

    pub(crate) fn note_relaid(&mut self, id: NodeId) {
        self.repaints.push(id);
    }

    pub(crate) fn take_repaints(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.repaints)
    }

    fn mark_stale(&mut self, id: NodeId) {
        if self.marked == Some(id) {
            return;
        }
        self.marked = Some(id);
        let mut current = Some(id);
        for _ in 0..ANCESTOR_LIMIT {
            let Some(node) = current else {
                return;
            };
            let index = node.index() as usize;
            let Some(stale) = self.stale.get_mut(index) else {
                return;
            };
            *stale = true;
            if let Some(unplaced) = self.unplaced.get_mut(index) {
                *unplaced = true;
            }
            current = self.parents.get(index).copied().flatten();
        }
    }

    pub(crate) fn stale(&self, id: NodeId) -> bool {
        self.stale.get(id.index() as usize).copied().unwrap_or(true)
    }

    pub(crate) fn clear_stale(&mut self, id: NodeId) {
        if let Some(stale) = self.stale.get_mut(id.index() as usize) {
            *stale = false;
        }
        self.marked = None;
    }

    pub(crate) fn unplaced(&self, id: NodeId) -> bool {
        self.unplaced
            .get(id.index() as usize)
            .copied()
            .unwrap_or(true)
    }

    pub(crate) fn clear_unplaced(&mut self, id: NodeId) {
        if let Some(unplaced) = self.unplaced.get_mut(id.index() as usize) {
            *unplaced = false;
        }
        self.marked = None;
    }

    pub(crate) fn set_parent(&mut self, id: NodeId, parent: Option<NodeId>) {
        let index = id.index() as usize;
        if self.parents.get(index).copied().flatten() == parent {
            return;
        }
        if let Some(held) = self.parents.get_mut(index) {
            *held = parent;
        }
        self.marked = None;
    }

    pub(crate) fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.parents.get(id.index() as usize).copied().flatten()
    }

    pub(crate) fn relaid_len(&self) -> usize {
        self.relaid.len()
    }

    pub(crate) fn relaid_since(&self, watermark: usize) -> &[NodeId] {
        self.relaid.get(watermark..).unwrap_or_default()
    }

    pub(crate) fn changed(&self) -> &[NodeId] {
        &self.changed
    }

    pub(crate) fn take_changed(&mut self) -> Vec<NodeId> {
        self.relaid.clear();
        let mut changed = std::mem::take(&mut self.changed);
        changed.sort_unstable_by_key(|id| id.0);
        changed.dedup();
        changed
    }

    pub(crate) fn take_everything(&mut self) -> bool {
        std::mem::take(&mut self.everything)
    }

    pub(crate) fn remove(&mut self, id: NodeId) {
        self.invalidate_node(id);
        if self.nodes[id.0 as usize].take().is_some() {
            self.live -= 1;
        }
    }
}
