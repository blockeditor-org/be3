use std::any::Any;
use std::cell::Cell;
use std::collections::HashMap;

use crate::base::child_list::{ChildHost, ChildList};
use crate::base::list::Direction;
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::painter::Painter;

use crate::callback::Callback;
use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, Rects, SpaceId, NodeOf};
use ::reactive::settle;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ScrollPosition {
    pub offset: f32,
    pub content: f32,
    pub viewport: f32,
    pub overscroll: f32,
}

impl ScrollPosition {
    pub const ZERO: Self = Self {
        offset: 0.0,
        content: 0.0,
        viewport: 0.0,
        overscroll: 0.0,
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

struct OffsetAnchor {
    id: NodeId,
    start: f32,
}

#[derive(Default)]
struct Extents {
    items: Vec<NodeId>,
    indices: HashMap<NodeId, usize>,
    lengths: Vec<f32>,
    ends: Vec<f32>,
    cross: Option<f32>,
    revision: u64,
    epoch: u64,
}

impl Extents {
    fn start(&self, index: usize) -> f32 {
        match index {
            0 => 0.0,
            _ => self.ends[index - 1],
        }
    }

    fn total(&self) -> f32 {
        self.ends.last().copied().unwrap_or(0.0)
    }

    fn first_ending_after(&self, at: f32) -> usize {
        self.ends.partition_point(|end| *end <= at)
    }

    fn sum_from(&mut self, from: usize) {
        self.ends.truncate(from);
        let mut end = self.start(from);
        for length in &self.lengths[from..] {
            end += length;
            self.ends.push(end);
        }
    }
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

pub struct OffsetNode {
    pub direction: Direction,
    pub items: ChildList<NodeId>,
    pub offset: f32,
    pub overscroll: f32,
    pub position: Option<ScrollPosition>,
    steered: Cell<bool>,
    anchor: Option<OffsetAnchor>,
    extents: Extents,
    pub on_change: Callback<ScrollPosition>,
    pub reported: Option<ScrollPosition>,
    translation: Vec2,
    host: Option<NodeId>,
}

impl Default for OffsetNode {
    fn default() -> Self {
        Self::new()
    }
}

impl OffsetNode {
    pub fn new() -> Self {
        Self {
            direction: Direction::Vertical,
            items: ChildList::default(),
            offset: 0.0,
            overscroll: 0.0,
            position: None,
            steered: Cell::new(false),
            anchor: None,
            extents: Extents::default(),
            on_change: Callback::empty(),
            reported: None,
            translation: Vec2::ZERO,
            host: None,
        }
    }

    fn item_length(&self, doc: &mut Document, painter: &Painter, item: NodeId, cross: f32) -> f32 {
        let offer = self.direction.axes(f32::INFINITY, cross);
        match doc.measured(item, offer) {
            Some(size) => self.direction.main(size),
            None => length(doc, painter, item, self.direction, cross),
        }
    }

    fn refresh(&mut self, doc: &mut Document, painter: &Painter, id: NodeId, cross: f32) -> bool {
        let stale = doc.arena.take_stale_children(id);
        let extents = &self.extents;
        if stale.overflowed
            || extents.cross != Some(cross)
            || extents.revision != self.items.revision()
            || extents.epoch != doc.arena.epoch
        {
            let items = self.items.nodes();
            let lengths: Vec<f32> = items
                .iter()
                .map(|item| self.item_length(doc, painter, *item, cross))
                .collect();
            let changed = lengths != self.extents.lengths;
            self.extents = Extents {
                indices: items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| (*item, index))
                    .collect(),
                items,
                lengths,
                ends: Vec::new(),
                cross: Some(cross),
                revision: self.items.revision(),
                epoch: doc.arena.epoch,
            };
            self.extents.sum_from(0);
            return changed;
        }
        let mut from = None;
        let mut unplaced = false;
        for item in stale.nodes {
            let Some(&index) = self.extents.indices.get(&item) else {
                continue;
            };
            let length = self.item_length(doc, painter, item, cross);
            if self.extents.lengths[index] != length {
                self.extents.lengths[index] = length;
                from = Some(from.map_or(index, |from: usize| from.min(index)));
            } else if doc.arena.unplaced(item) {
                unplaced = true;
            }
        }
        if let Some(from) = from {
            self.extents.sum_from(from);
        }
        from.is_some() || unplaced
    }

    fn nodes(&self) -> Vec<NodeId> {
        self.items.nodes()
    }

    fn anchored_offset(&self) -> f32 {
        let Some(anchor) = &self.anchor else {
            return self.offset;
        };
        self.extents
            .indices
            .get(&anchor.id)
            .map_or(self.offset, |index| {
                self.extents.start(*index) - anchor.start
            })
    }

    fn remember_anchor(&mut self) {
        let index = self.extents.first_ending_after(self.offset);
        self.anchor = self.extents.items.get(index).map(|id| OffsetAnchor {
            id: *id,
            start: self.extents.start(index) - self.offset,
        });
    }

    fn revealing(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        id: NodeId,
        rect: Rect,
        item: NodeId,
        focused: NodeId,
    ) -> Option<f32> {
        let (_, cross) = self.direction.main_and_cross(rect.size());
        self.refresh(doc, painter, id, cross);
        let index = *self.extents.indices.get(&item)?;
        let start =
            self.direction.main(rect.min.to_vec2()) + self.extents.start(index) - self.offset;
        let placed = item_rect(self.direction, rect, start, self.extents.lengths[index]);
        let rects = Rects::default();
        crate::layout::layout(doc, painter, item, placed, &rects);
        let target = rects.get(&focused).unwrap_or(placed);
        revealed_offset(self.direction, rect, target, self.offset)
    }

    fn settled(&mut self, main: f32) -> ScrollPosition {
        let content = self.extents.total();
        let position = ScrollPosition {
            offset: self.offset.clamp(0.0, (content - main).max(0.0)),
            content,
            viewport: main,
            overscroll: self.overscroll,
        };
        self.offset = position.offset;
        position
    }

    fn place(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &Rects,
        position: ScrollPosition,
        host: NodeId,
    ) {
        let main = self.direction.main(rect.size());
        let start = self.direction.main(rect.min.to_vec2());
        let offset = doc.pixel_grid().snap(position.offset + self.overscroll);
        self.translation = self.direction.axes(-offset, 0.0);
        self.host = Some(host);
        let entered = doc.enter_space(host, 1, painter, self.translation, rect, out);
        doc.enter_scroll_host(host);
        let extents = &self.extents;
        for index in extents.first_ending_after(offset)..extents.items.len() {
            let cursor = start + extents.start(index);
            if cursor - offset >= start + main {
                break;
            }
            let length = extents.lengths[index];
            if cursor - offset + length > start {
                crate::layout::layout(
                    doc,
                    &entered,
                    extents.items[index],
                    item_rect(self.direction, rect, cursor, length),
                    out,
                );
            }
        }
        doc.leave_scroll_host();
    }
}

fn revealed_offset(direction: Direction, viewport: Rect, item: Rect, offset: f32) -> Option<f32> {
    let length = direction.main(viewport.size());
    let start = direction.main(item.min - viewport.min) + offset;
    let end = direction.main(item.max - viewport.min) + offset;
    if start <= offset && end >= offset + length {
        return None;
    }
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

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let host = doc.laying_out().expect("an offset is laid out as itself");
        let (main, cross) = self.direction.main_and_cross(rect.size());
        let carried = doc.take_scroll_shift(host);
        self.refresh(doc, painter, host, cross);
        self.offset = (self.anchored_offset() + carried).max(0.0);
        let mut position = self.settled(main);
        let base = doc.placing_len();
        self.place(doc, painter, rect, out, position, host);
        let shift = doc.take_scroll_shift(host);
        let changed = self.refresh(doc, painter, host, cross);
        if shift != 0.0 || changed {
            self.offset = (self.offset + shift).max(0.0);
            position = self.settled(main);
            doc.rewind_placing(base);
            self.place(doc, painter, rect, out, position, host);
            doc.take_scroll_shift(host);
        }
        if shift != 0.0 || carried != 0.0 {
            self.anchor = None;
        }
        if self.anchor.is_none() {
            self.remember_anchor();
        }
        self.position = Some(position);
        if !self.on_change.is_empty() && self.reported != Some(position) {
            self.reported = Some(position);
            let on_change = &self.on_change;
            settle(|| on_change.call(position));
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        let Some(host) = self.host else {
            return;
        };
        let entered = painter.shifted(Some(SpaceId::inside(host, 1)), self.translation, rect);
        for item in self.items.iter() {
            if rects.contains_key(item) {
                crate::paint::paint(doc, &entered, rects, *item);
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

    fn tracks_stale_children(&self) -> bool {
        true
    }

    fn kind(&self) -> &'static str {
        "offset"
    }

    fn detail(&self) -> Option<String> {
        (self.direction == Direction::Horizontal).then(|| "horizontal".to_string())
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

pub fn item_rect(direction: Direction, rect: Rect, start: f32, length: f32) -> Rect {
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
    pub fn create_offset(&mut self) -> NodeOf<OffsetNode> {
        self.arena.insert(OffsetNode::new())
    }

    pub fn set_offset_direction(&mut self, offset: NodeOf<OffsetNode>, direction: Direction) {
        if self.arena.get_as::<OffsetNode>(offset).direction == direction {
            return;
        }
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        node.direction = direction;
        node.anchor = None;
        node.extents = Extents::default();
    }

    pub fn offset_value(&self, offset: NodeOf<OffsetNode>) -> f32 {
        self.arena.get_as::<OffsetNode>(offset).offset
    }

    pub fn offset_position(&self, offset: NodeOf<OffsetNode>) -> Option<ScrollPosition> {
        self.arena.get_as::<OffsetNode>(offset).position
    }

    pub fn set_offset_value(&mut self, offset: NodeOf<OffsetNode>, value: f32) {
        let node = self.arena.get_as::<OffsetNode>(offset);
        node.steered.set(true);
        if node.offset == value && node.anchor.is_none() {
            return;
        }
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        node.offset = value;
        node.anchor = None;
    }

    pub fn drive_offset(&mut self, offset: NodeOf<OffsetNode>, value: f32) {
        if self.arena.get_as::<OffsetNode>(offset).offset == value {
            return;
        }
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        node.offset = value;
        node.anchor = None;
    }

    pub fn take_offset_steered(&self, offset: NodeOf<OffsetNode>) -> bool {
        self.arena
            .get_as::<OffsetNode>(offset)
            .steered
            .replace(false)
    }

    pub fn set_offset_overscroll(&mut self, offset: NodeOf<OffsetNode>, overscroll: f32) {
        if self.arena.get_as::<OffsetNode>(offset).overscroll == overscroll {
            return;
        }
        self.arena.get_mut_as::<OffsetNode>(offset).overscroll = overscroll;
    }

    pub fn first_offset_within(&self, id: NodeId) -> Option<NodeOf<OffsetNode>> {
        if !self.contains(id) {
            return None;
        }
        if let Some(offset) = self.arena.kind_of::<OffsetNode>(id) {
            return Some(offset);
        }
        self.arena
            .get(id)
            .children()
            .into_iter()
            .find_map(|child| self.first_offset_within(child))
    }

    pub fn scroll_offset(&self, scroll: NodeId) -> f32 {
        self.first_offset_within(scroll)
            .map_or(0.0, |offset| self.offset_value(offset))
    }

    pub fn set_scroll_offset(&mut self, scroll: NodeId, offset: f32) {
        if let Some(node) = self.first_offset_within(scroll) {
            self.set_offset_value(node, offset);
        }
    }

    pub fn scroll_overscroll(&self, scroll: NodeId) -> f32 {
        self.first_offset_within(scroll).map_or(0.0, |offset| {
            self.arena.get_as::<OffsetNode>(offset).overscroll
        })
    }

    pub fn set_offset_on_change(
        &mut self,
        offset: NodeOf<OffsetNode>,
        handler: impl FnMut(ScrollPosition) + 'static,
    ) {
        self.arena
            .touch_mut_as::<OffsetNode>(offset)
            .on_change
            .set(handler);
    }
}

impl Document {
    pub fn reveal_offset_index(&mut self, offset: NodeOf<OffsetNode>, index: usize) {
        let Some(&item) = self.arena.get_as::<OffsetNode>(offset).items.get(index) else {
            return;
        };
        self.reveal_offset_item(offset, item);
    }

    fn reveal_offset_item(&mut self, offset: NodeOf<OffsetNode>, item: NodeId) {
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
        if self.delivering() {
            self.deferred_reveals.push(node);
            return;
        }
        let Some(root) = self.root else {
            return;
        };
        let mut path = Vec::new();
        if !self.focus_path(root, node, &mut path) {
            return;
        }
        for id in path.into_iter().rev().skip(1) {
            if let Some(offset) = self.arena.kind_of::<OffsetNode>(id) {
                self.reveal_offset_item(offset, node);
                return;
            }
        }
    }

    pub fn reveal_focus(&mut self, painter: &Painter) {
        let (Some(root), Some(focused)) = (self.root, self.focused) else {
            return;
        };
        let mut path = Vec::new();
        if !self.focus_path(root, focused, &mut path) {
            return;
        }
        for pair in path.windows(2).rev() {
            let (offset, item) = (pair[0], pair[1]);
            let Some(offset) = self.arena.kind_of::<OffsetNode>(offset) else {
                continue;
            };
            let Some(rect) = self.node_rect(offset) else {
                continue;
            };
            let mut element = self.arena.take(offset);
            let revealed = element
                .as_any_mut()
                .downcast_mut::<OffsetNode>()
                .and_then(|node| node.revealing(self, painter, offset.id(), rect, item, focused));
            self.arena.put_back(offset, element);
            if let Some(revealed) = revealed {
                self.set_offset_value(offset, revealed);
            }
        }
    }
}
