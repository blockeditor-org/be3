use std::any::Any;
use std::cell::Cell;

use crate::base::list::Direction;
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::painter::Painter;

use crate::callback::Callback;
use crate::document::Document;
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects, SpaceId};
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

pub struct OffsetNode {
    pub direction: Direction,
    pub child: NodeId,
    pub offset: f32,
    pub overscroll: f32,
    pub fits: bool,
    pub position: Option<ScrollPosition>,
    steered: Cell<bool>,
    anchored: bool,
    pub on_change: Callback<ScrollPosition>,
    pub reported: Option<ScrollPosition>,
    translation: Vec2,
    host: Option<NodeId>,
}

impl OffsetNode {
    pub fn new(child: NodeId) -> Self {
        Self {
            direction: Direction::Vertical,
            child,
            offset: 0.0,
            overscroll: 0.0,
            fits: false,
            position: None,
            steered: Cell::new(false),
            anchored: false,
            on_change: Callback::empty(),
            reported: None,
            translation: Vec2::ZERO,
            host: None,
        }
    }

    fn content_length(&self, doc: &mut Document, painter: &Painter, cross: f32) -> f32 {
        self.direction.main(crate::layout::measure(
            doc,
            painter,
            self.child,
            self.direction.axes(f32::INFINITY, cross),
        ))
    }

    fn settled(&mut self, main: f32, content: f32) -> ScrollPosition {
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
        let offset = doc.pixel_grid().snap(position.offset + self.overscroll);
        self.translation = self.direction.axes(-offset, 0.0);
        self.host = Some(host);
        let entered = doc.enter_space(host, 1, painter, self.translation, rect, out);
        doc.enter_scroll_host(host);
        let start = self.direction.main(rect.min.to_vec2());
        crate::layout::layout(
            doc,
            &entered,
            self.child,
            item_rect(self.direction, rect, start, position.content),
            out,
        );
        doc.leave_scroll_host();
    }

    fn content_start(&self, out: &Rects, host: NodeId, id: NodeId) -> Option<f32> {
        let rect = out.get(&id)?;
        let space = out.offset(Some(SpaceId::inside(host, 1)));
        Some(self.direction.main(rect.min.to_vec2() - space))
    }

    fn drift(
        &self,
        doc: &Document,
        out: &Rects,
        host: NodeId,
        anchor: Option<OffsetAnchor>,
    ) -> f32 {
        let Some(anchor) = anchor.filter(|anchor| doc.contains(anchor.id)) else {
            return 0.0;
        };
        self.content_start(out, host, anchor.id)
            .map_or(0.0, |start| start - anchor.start)
    }

    fn anchor(&self, doc: &Document, out: &Rects, host: NodeId) -> Option<OffsetAnchor> {
        if !self.anchored {
            return None;
        }
        let mut anchor = self.child;
        for _ in 0..ANCHOR_DEPTH {
            if doc.is_culled(anchor) || !doc.arena.get(anchor).passes_scroll_anchor() {
                break;
            }
            let next = doc.arena.get(anchor).children().into_iter().find(|child| {
                out.get(child).is_some_and(|rect| {
                    self.content_start(out, host, *child)
                        .is_some_and(|start| start + self.direction.main(rect.size()) > self.offset)
                })
            });
            match next {
                Some(next) => anchor = next,
                None => break,
            }
        }
        self.content_start(out, host, anchor)
            .map(|start| OffsetAnchor { id: anchor, start })
    }
}

const ANCHOR_DEPTH: usize = 16;

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
        let size = crate::layout::measure(
            doc,
            painter,
            self.child,
            self.direction.axes(f32::INFINITY, cross),
        );
        let (length, content) = self.direction.main_and_cross(size);
        let length = match self.fits {
            true => length,
            false => 0.0,
        };
        self.direction.axes(length, content)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let host = doc.laying_out().expect("an offset is laid out as itself");
        let (main, cross) = self.direction.main_and_cross(rect.size());
        let carried = doc.take_scroll_shift(host);
        let anchor = self.anchor(doc, out, host);
        self.anchored = true;
        let content = self.content_length(doc, painter, cross);
        self.offset = (self.offset + carried).max(0.0);
        let mut position = self.settled(main, content);
        let base = doc.placing_len();
        self.place(doc, painter, rect, out, position, host);
        let shift = doc.take_scroll_shift(host) + self.drift(doc, out, host, anchor);
        let measured = self.content_length(doc, painter, cross);
        if shift != 0.0 || measured != content || doc.arena.unplaced(self.child) {
            self.offset = (self.offset + shift).max(0.0);
            position = self.settled(main, measured);
            doc.rewind_placing(base);
            self.place(doc, painter, rect, out, position, host);
            doc.take_scroll_shift(host);
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
        if !rects.contains_key(&self.child) {
            return;
        }
        let entered = painter.shifted(Some(SpaceId::inside(host, 1)), self.translation, rect);
        crate::paint::paint(doc, &entered, rects, self.child);
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
        children.push(self.child);
    }

    fn children(&self) -> Vec<NodeId> {
        vec![self.child]
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
    pub fn create_offset(&mut self, child: NodeId) -> NodeOf<OffsetNode> {
        self.arena.insert(OffsetNode::new(child))
    }

    pub fn set_offset_direction(&mut self, offset: NodeOf<OffsetNode>, direction: Direction) {
        if self.arena.get_as::<OffsetNode>(offset).direction == direction {
            return;
        }
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        node.direction = direction;
        node.anchored = false;
    }

    pub fn set_offset_fits(&mut self, offset: NodeOf<OffsetNode>, fits: bool) {
        if self.arena.get_as::<OffsetNode>(offset).fits != fits {
            self.arena.get_mut_as::<OffsetNode>(offset).fits = fits;
        }
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
        if node.offset == value && !node.anchored {
            return;
        }
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        node.offset = value;
        node.anchored = false;
    }

    pub fn drive_offset(&mut self, offset: NodeOf<OffsetNode>, value: f32) {
        if self.arena.get_as::<OffsetNode>(offset).offset == value {
            return;
        }
        let node = self.arena.get_mut_as::<OffsetNode>(offset);
        node.offset = value;
        node.anchored = false;
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
        let placed = path
            .iter()
            .rev()
            .copied()
            .find(|id| self.node_rect(*id).is_some())
            .unwrap_or(node);
        for id in path.into_iter().rev().skip(1) {
            if let Some(offset) = self.arena.kind_of::<OffsetNode>(id) {
                self.reveal_offset_item(offset, placed);
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
        let Some(target) = self.focus_target_rect(painter, focused) else {
            return;
        };
        for id in path.into_iter().rev().skip(1) {
            let Some(offset) = self.arena.kind_of::<OffsetNode>(id) else {
                continue;
            };
            let Some(viewport) = self.node_rect(offset) else {
                continue;
            };
            let direction = self.arena.get_as::<OffsetNode>(offset).direction;
            let current = self.offset_value(offset);
            if let Some(revealed) = revealed_offset(direction, viewport, target, current) {
                self.set_offset_value(offset, revealed);
            }
        }
    }

    fn focus_target_rect(&mut self, painter: &Painter, focused: NodeId) -> Option<Rect> {
        let Some(culled) = self.culled_ancestor(focused) else {
            return self.node_rect(focused);
        };
        let placed = self.node_rect(culled)?;
        let rects = Rects::default();
        crate::layout::layout(self, painter, culled, placed, &rects);
        Some(rects.get(&focused).unwrap_or(placed))
    }
}
