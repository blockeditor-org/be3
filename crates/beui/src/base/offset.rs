use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use crate::base::child_list::{ChildHost, ChildList, SlotId};
use crate::base::list::Direction;
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::painter::Painter;
use crate::reactive::KeyedItems;

use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, NodeMap};
use crate::reactive::{
    Callback, Children, Prop, RenderFn, ScopeContext, create_effect, create_signal, owner_scope,
    settle, with_document,
};
use beui_macros::component;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScrollPosition {
    pub offset: f32,
    pub content: f32,
    pub viewport: f32,
}

impl ScrollPosition {
    pub const ZERO: Self = Self {
        offset: 0.0,
        content: 0.0,
        viewport: 0.0,
    };

    pub fn max_offset(&self) -> f32 {
        (self.content - self.viewport).max(0.0)
    }
}

impl Default for ScrollPosition {
    fn default() -> Self {
        Self::ZERO
    }
}

type VirtualRows = KeyedItems<usize, NodeId>;

pub(crate) struct VirtualItems {
    pub(crate) count: usize,
    pub(crate) estimated: f32,
    pub(crate) first: usize,
    slot: SlotId,
    rows: Rc<VirtualRows>,
    owner: Option<ScopeContext>,
}

enum OffsetAnchor {
    Node { id: NodeId, start: f32 },
    VirtualItem { index: usize, start: f32 },
}

impl ChildHost for OffsetNode {
    type Stored = NodeId;

    fn children(&mut self) -> &mut ChildList<NodeId> {
        &mut self.items
    }

    fn children_changed(&mut self) {
        self.anchor = None;
    }
}

pub(crate) struct OffsetNode {
    pub(crate) direction: Direction,
    pub(crate) items: ChildList<NodeId>,
    pub(crate) virtual_items: Option<VirtualItems>,
    pub(crate) offset: f32,
    pub(crate) overscroll: f32,
    pub(crate) position: Option<ScrollPosition>,
    steered: Cell<bool>,
    anchor: Option<OffsetAnchor>,
    pub(crate) on_change: Callback<ScrollPosition>,
    pub(crate) reported: Option<ScrollPosition>,
}

impl OffsetNode {
    pub(crate) fn new() -> Self {
        Self {
            direction: Direction::Vertical,
            items: ChildList::default(),
            virtual_items: None,
            offset: 0.0,
            overscroll: 0.0,
            position: None,
            steered: Cell::new(false),
            anchor: None,
            on_change: Callback::empty(),
            reported: None,
        }
    }

    fn lengths(&self, doc: &mut Document, painter: &Painter, cross: f32) -> Vec<f32> {
        let offer = self.direction.axes(f32::INFINITY, cross);
        self.items
            .nodes()
            .into_iter()
            .map(|item| match doc.measured(item, offer) {
                Some(size) => self.direction.main(size),
                None => length(doc, painter, item, self.direction, cross),
            })
            .collect()
    }

    fn nodes(&self) -> Vec<NodeId> {
        self.items.nodes()
    }

    fn content(&self, measured: f32) -> f32 {
        match &self.virtual_items {
            Some(items) => items.count as f32 * items.estimated,
            None => measured,
        }
    }

    fn leading(&self, offset: f32) -> f32 {
        match &self.virtual_items {
            Some(items) => items.first as f32 * items.estimated - offset,
            None => -offset,
        }
    }

    fn anchored_offset(&self, lengths: &[f32]) -> f32 {
        match &self.anchor {
            Some(OffsetAnchor::Node { id, start }) => self
                .items
                .iter()
                .position(|item| item == id)
                .map_or(self.offset, |index| {
                    lengths[..index].iter().sum::<f32>() - start
                }),
            Some(OffsetAnchor::VirtualItem { index, start }) => self
                .virtual_items
                .as_ref()
                .map_or(self.offset, |items| *index as f32 * items.estimated - start),
            None => self.offset,
        }
    }

    fn remember_anchor(&mut self, doc: &mut Document, painter: &Painter, cross: f32) {
        if let Some(items) = &self.virtual_items {
            self.anchor = (items.count > 0).then_some(OffsetAnchor::VirtualItem {
                index: items.first,
                start: items.first as f32 * items.estimated - self.offset,
            });
            return;
        }
        let mut start = -self.offset;
        self.anchor = None;
        for (&id, length) in self.items.iter().zip(self.lengths(doc, painter, cross)) {
            if start + length > 0.0 {
                self.anchor = Some(OffsetAnchor::Node { id, start });
                break;
            }
            start += length;
        }
    }

    fn revealing(
        &self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        item: NodeId,
        focused: NodeId,
    ) -> Option<f32> {
        let (_, cross) = self.direction.main_and_cross(rect.size());
        let lengths = self.lengths(doc, painter, cross);
        let index = self.items.iter().position(|id| *id == item)?;
        let start = self.direction.main(rect.min.to_vec2())
            + lengths[..index].iter().sum::<f32>()
            + self.leading(0.0)
            - self.offset;
        let placed = item_rect(self.direction, rect, start, lengths[index]);
        let mut rects = NodeMap::default();
        crate::layout::layout(doc, painter, item, placed, &mut rects);
        let target = rects.get(&focused).copied().unwrap_or(placed);
        revealed_offset(self.direction, rect, target, self.offset)
    }

    fn realize(&mut self, doc: &mut Document, rect: Rect) {
        let Some(items) = self.virtual_items.take() else {
            return;
        };
        let VirtualItems {
            count,
            estimated,
            slot,
            rows,
            owner,
            ..
        } = items;
        let (main, _) = self.direction.main_and_cross(rect.size());
        let first = if estimated > 0.0 {
            ((self.offset / estimated) as usize).min(count.saturating_sub(1))
        } else {
            0
        };
        let leading = first as f32 * estimated - self.offset;
        let fit = match estimated > 0.0 {
            true => ((main - leading) / estimated).ceil().max(0.0) as usize,
            false => 0,
        };
        let last = first.saturating_add(fit).min(count);
        let range: Vec<usize> = (first..last).collect();
        let mapping = match owner.as_ref().filter(|owner| owner.is_alive()) {
            Some(owner) => settle(|| owner.run(|| rows.map(range))),
            None => settle(|| rows.map(range)),
        };
        mapping.commit(|realized, evicted| {
            self.items.fill(slot, realized);
            for item in evicted {
                doc.remove_node(item);
            }
        });
        self.virtual_items = Some(VirtualItems {
            count,
            estimated,
            first,
            slot,
            rows,
            owner,
        });
    }
}

fn revealed_offset(direction: Direction, viewport: Rect, item: Rect, offset: f32) -> Option<f32> {
    let length = direction.main(viewport.size());
    let start = direction.main(item.min - viewport.min) + offset;
    let end = direction.main(item.max - viewport.min) + offset;
    let revealed = if start < offset {
        start
    } else if end > offset + length {
        (end - length).min(start)
    } else {
        return None;
    };
    Some(revealed.max(0.0))
}

impl Element for OffsetNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        let (_, cross) = self.direction.main_and_cross(available);
        let content = self
            .nodes()
            .into_iter()
            .map(|item| {
                let size = crate::layout::measure(
                    doc,
                    painter,
                    item,
                    self.direction.axes(f32::INFINITY, cross),
                );
                self.direction.main_and_cross(size).1
            })
            .fold(0.0_f32, f32::max);
        self.direction.axes(0.0, content)
    }

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut NodeMap<Rect>,
    ) {
        let (main, cross) = self.direction.main_and_cross(rect.size());
        let virtualised = self.virtual_items.is_some();
        let mut lengths = match virtualised {
            true => Vec::new(),
            false => self.lengths(doc, painter, cross),
        };
        self.offset = self.anchored_offset(&lengths).max(0.0);
        if virtualised {
            self.realize(doc, rect);
            lengths = self.lengths(doc, painter, cross);
        }
        let content = self.content(lengths.iter().sum());
        let position = ScrollPosition {
            offset: self.offset.clamp(0.0, (content - main).max(0.0)),
            content,
            viewport: main,
        };
        self.offset = position.offset;
        let grid = doc.pixel_grid();
        let offset = grid.snap(position.offset + self.overscroll);
        let start = self.direction.main(rect.min.to_vec2());
        let mut cursor = start + grid.snap(self.leading(offset));
        let clipped = painter.with_clip_rect(rect);
        for (&item, length) in self.items.iter().zip(&lengths) {
            if cursor >= start + main {
                break;
            }
            if cursor + length > start {
                crate::layout::layout(
                    doc,
                    &clipped,
                    item,
                    item_rect(self.direction, rect, cursor, *length),
                    out,
                );
            }
            cursor += length;
        }
        if self.anchor.is_none() {
            self.remember_anchor(doc, painter, cross);
        }
        self.position = Some(position);
        if !self.on_change.is_empty() && self.reported != Some(position) {
            self.reported = Some(position);
            let on_change = &self.on_change;
            settle(|| on_change.call(position));
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &NodeMap<Rect>, rect: Rect) {
        let clipped = painter.with_clip_rect(rect);
        for item in self.items.iter() {
            if rects.contains_key(item) {
                crate::paint::paint(doc, &clipped, rects, *item);
            }
        }
    }

    fn interact(
        &mut self,
        _doc: &mut Document,
        _painter: &Painter,
        _input: &InteractInput,
        _id: NodeId,
        _rect: Rect,
        _focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        children.extend(self.items.iter().copied());
    }

    fn children(&self) -> Vec<NodeId> {
        self.nodes()
    }

    fn kind(&self) -> &'static str {
        "offset"
    }

    fn detail(&self) -> Option<String> {
        let horizontal = (self.direction == Direction::Horizontal).then_some("horizontal");
        let realized = self.virtual_items.as_ref().map(|items| {
            format!(
                "{}..{} of {}",
                items.first,
                items.first + self.items.len(),
                items.count
            )
        });
        let detail = [horizontal.map(str::to_string), realized]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
        (!detail.is_empty()).then_some(detail)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn length(
    doc: &mut Document,
    painter: &Painter,
    item: NodeId,
    direction: Direction,
    cross: f32,
) -> f32 {
    direction.main(crate::layout::measure(
        doc,
        painter,
        item,
        direction.axes(f32::INFINITY, cross),
    ))
}

fn item_rect(direction: Direction, rect: Rect, start: f32, length: f32) -> Rect {
    match direction {
        Direction::Horizontal => {
            Rect::from_min_size(pos2(start, rect.top()), vec2(length, rect.height()))
        }
        Direction::Vertical => {
            Rect::from_min_size(pos2(rect.left(), start), vec2(rect.width(), length))
        }
    }
}

impl Document {
    pub(crate) fn create_offset(&mut self) -> NodeId {
        self.arena.insert(OffsetNode::new())
    }

    pub(crate) fn set_offset_direction(&mut self, offset: NodeId, direction: Direction) {
        if self.arena.get_as::<OffsetNode>(offset).direction == direction {
            return;
        }
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        node.direction = direction;
        node.anchor = None;
    }

    pub(crate) fn set_offset_virtual_items(
        &mut self,
        offset: NodeId,
        count: usize,
        estimated_height: f32,
        build: impl Fn(usize) -> NodeId + 'static,
    ) {
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        if matches!(node.anchor, Some(OffsetAnchor::Node { .. })) {
            node.anchor = None;
        }
        let items = node.items.take_all();
        for item in items {
            self.remove_node(item);
        }
        let owner = owner_scope();
        let slot = self.arena.get_mut_as::<OffsetNode>(offset).items.open();
        self.arena.get_mut_as::<OffsetNode>(offset).virtual_items = Some(VirtualItems {
            count,
            estimated: estimated_height.max(0.0),
            first: 0,
            slot,
            rows: Rc::new(KeyedItems::new(build)),
            owner,
        });
    }

    pub(crate) fn offset_value(&self, offset: NodeId) -> f32 {
        self.arena.get_as::<OffsetNode>(offset).offset
    }

    pub(crate) fn offset_position(&self, offset: NodeId) -> Option<ScrollPosition> {
        self.arena.get_as::<OffsetNode>(offset).position
    }

    pub(crate) fn set_offset_value(&mut self, offset: NodeId, value: f32) {
        let node = self.arena.get_as::<OffsetNode>(offset);
        node.steered.set(true);
        if node.offset == value && node.anchor.is_none() {
            return;
        }
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        node.offset = value;
        node.anchor = None;
    }

    pub(crate) fn drive_offset(&mut self, offset: NodeId, value: f32) {
        if self.arena.get_as::<OffsetNode>(offset).offset == value {
            return;
        }
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        node.offset = value;
        node.anchor = None;
    }

    pub(crate) fn take_offset_steered(&self, offset: NodeId) -> bool {
        self.arena
            .get_as::<OffsetNode>(offset)
            .steered
            .replace(false)
    }

    pub(crate) fn set_offset_overscroll(&mut self, offset: NodeId, overscroll: f32) {
        if self.arena.get_as::<OffsetNode>(offset).overscroll == overscroll {
            return;
        }
        self.arena.get_mut_as::<OffsetNode>(offset).overscroll = overscroll;
    }

    pub(crate) fn first_offset_within(&self, id: NodeId) -> Option<NodeId> {
        if !self.contains(id) {
            return None;
        }
        let element = self.arena.get(id);
        if element.as_any().is::<OffsetNode>() {
            return Some(id);
        }
        element
            .children()
            .into_iter()
            .find_map(|child| self.first_offset_within(child))
    }

    pub fn scroll_offset(&self, scroll: NodeId) -> f32 {
        self.first_offset_within(scroll)
            .map_or(0.0, |offset| self.offset_value(offset))
    }

    #[cfg(test)]
    pub(crate) fn set_scroll_offset(&mut self, scroll: NodeId, offset: f32) {
        if let Some(node) = self.first_offset_within(scroll) {
            self.set_offset_value(node, offset);
        }
    }

    #[cfg(test)]
    pub(crate) fn scroll_overscroll(&self, scroll: NodeId) -> f32 {
        self.first_offset_within(scroll).map_or(0.0, |offset| {
            self.arena.get_as::<OffsetNode>(offset).overscroll
        })
    }

    pub(crate) fn set_offset_on_change(
        &mut self,
        offset: NodeId,
        handler: impl FnMut(ScrollPosition) + 'static,
    ) {
        self.arena
            .get_mut_as::<OffsetNode>(offset)
            .on_change
            .set(handler);
    }
}

impl Document {
    fn reveal_offset_index(&mut self, offset: NodeId, index: usize) {
        let Some(&item) = self.arena.get_as::<OffsetNode>(offset).items.get(index) else {
            return;
        };
        self.reveal_offset_item(offset, item);
    }

    fn reveal_offset_item(&mut self, offset: NodeId, item: NodeId) {
        let (Some(viewport), Some(item)) = (self.node_rect(offset), self.node_rect(item)) else {
            return;
        };
        let direction = self.arena.get_as::<OffsetNode>(offset).direction;
        let current = self.offset_value(offset);
        let Some(revealed) = revealed_offset(direction, viewport, item, current) else {
            return;
        };
        self.set_offset_value(offset, revealed);
    }

    pub fn reveal_node(&mut self, node: NodeId) {
        let Some(root) = self.root else {
            return;
        };
        let mut path = Vec::new();
        if !self.focus_path(root, node, &mut path) {
            return;
        }
        for id in path.into_iter().rev().skip(1) {
            if self.arena.get(id).as_any().is::<OffsetNode>() {
                self.reveal_offset_item(id, node);
                return;
            }
        }
    }

    pub(crate) fn reveal_focus(&mut self, painter: &Painter) {
        let (Some(root), Some(focused)) = (self.root, self.focused) else {
            return;
        };
        let mut path = Vec::new();
        if !self.focus_path(root, focused, &mut path) {
            return;
        }
        for pair in path.windows(2).rev() {
            let (offset, item) = (pair[0], pair[1]);
            if !self.arena.get(offset).as_any().is::<OffsetNode>() {
                continue;
            }
            let Some(rect) = self.node_rect(offset) else {
                continue;
            };
            let element = self.arena.take(offset);
            let revealed = element
                .as_any()
                .downcast_ref::<OffsetNode>()
                .and_then(|node| node.revealing(self, painter, rect, item, focused));
            self.arena.put_back(offset, element);
            if let Some(revealed) = revealed {
                self.set_offset_value(offset, revealed);
            }
        }
    }
}

#[component]
pub fn VirtualOffset(
    count: Prop<usize>,
    item_size: Prop<f32>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    #[prop(children)] item: Option<RenderFn<usize>>,
    on_change: Callback<ScrollPosition>,
) -> NodeId {
    let offset = create_offset(direction, on_change);
    let item = item.expect("virtual_offset requires an `item` builder");
    let (count_read, set_count) = create_signal(0);
    let (height_read, set_height) = create_signal(0.0);
    create_effect(move || set_count.set(count.get()));
    create_effect(move || set_height.set(item_size.get()));
    create_effect(move || {
        let (count, height) = (count_read.get(), height_read.get());
        let item = item.clone();
        with_document(|document| {
            document.set_offset_virtual_items(offset, count, height, move |index| item.call(index));
        });
    });
    offset
}

#[component]
pub fn Offset(
    #[prop(default = 0.0)] offset: Prop<f32>,
    #[prop(default = None)] reveal: Prop<Option<usize>>,
    #[prop(default = Direction::Vertical)] direction: Prop<Direction>,
    on_change: Callback<ScrollPosition>,
    children: Children<NodeId>,
) -> NodeId {
    let node = create_offset(direction, on_change);
    children.mount(node);
    create_effect(move || with_document(|document| document.set_offset_value(node, offset.get())));
    create_effect(move || {
        let index = reveal.get();
        let Some(index) = index else {
            return;
        };
        with_document(|document| document.reveal_offset_index(node, index));
    });
    node
}

fn create_offset(direction: Prop<Direction>, on_change: Callback<ScrollPosition>) -> NodeId {
    let offset = with_document(|document| {
        let offset = document.create_offset();
        document.set_offset_on_change(offset, move |position| on_change.call(position));
        offset
    });
    create_effect(move || {
        with_document(|document| document.set_offset_direction(offset, direction.get()))
    });
    offset
}
