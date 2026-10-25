use std::any::Any;

use crate::base::list::Direction;
use crate::geometry::{Rect, Vec2, pos2, vec2};
use crate::painter::Painter;

use crate::base::canvas::CanvasNode;
use crate::callback::Callback;
use crate::document::Document;
use crate::node::{Element, NodeId, NodeOf, Rects, SpaceId};

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

    pub fn clamped(&self) -> f32 {
        self.offset.clamp(0.0, self.max_offset())
    }

    pub fn overscroll(&self) -> f32 {
        self.offset - self.clamped()
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
    pub child: Option<NodeId>,
    pub offset: f32,
    pub fits: bool,
    pub position: Option<ScrollPosition>,
    anchored: bool,
    pub on_change: Callback<ScrollPosition>,
    pub on_steer: Callback<()>,
    pub reported: Option<ScrollPosition>,
    pub follows_end: bool,
    pinned: bool,
    to_end: bool,
    translation: Vec2,
    host: Option<NodeId>,
}

impl OffsetNode {
    pub fn new(child: NodeId) -> Self {
        Self {
            direction: Direction::Vertical,
            child: Some(child),
            offset: 0.0,
            fits: false,
            position: None,
            anchored: false,
            on_change: Callback::empty(),
            on_steer: Callback::empty(),
            reported: None,
            follows_end: false,
            pinned: true,
            to_end: false,
            translation: Vec2::ZERO,
            host: None,
        }
    }

    fn content_length(&self, doc: &mut Document, painter: &Painter, cross: f32) -> f32 {
        let Some(child) = self.child else {
            return 0.0;
        };
        self.direction.main(crate::layout::measure(
            doc,
            painter,
            child,
            self.direction.axes(f32::INFINITY, cross),
        ))
    }

    fn snaps_to_end(&self) -> bool {
        self.pinned && (self.follows_end || self.to_end)
    }

    fn end(&self, main: f32, content: f32) -> f32 {
        let past = self
            .position
            .filter(|_| !self.to_end)
            .map_or(0.0, |position| {
                (self.offset - position.max_offset()).max(0.0)
            });
        (content - main).max(0.0) + past
    }

    fn moved_to(&mut self, value: f32) {
        self.offset = value;
        self.anchored = false;
        self.to_end = false;
        self.pinned = self
            .position
            .is_none_or(|position| value >= position.max_offset() - END_SLACK);
    }

    fn settled(&self, main: f32, content: f32) -> ScrollPosition {
        ScrollPosition {
            offset: self.offset,
            content,
            viewport: main,
        }
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
        let offset = doc.pixel_grid().snap(position.offset);
        self.translation = self.direction.axes(-offset, 0.0);
        self.host = Some(host);
        let entered = doc.enter_space(host, 1, painter, self.translation, rect, out);
        let start = self.direction.main(rect.min.to_vec2());
        if let Some(child) = self.child {
            crate::layout::layout(
                doc,
                &entered,
                child,
                item_rect(self.direction, rect, start, position.content),
                out,
            );
        }
    }

    fn content_start(&self, out: &Rects, host: NodeId, id: NodeId) -> Option<f32> {
        let rect = out.get(&id)?;
        let space = out.transform(Some(SpaceId::inside(host, 1)));
        Some(self.direction.main(space.local_pos(rect.min).to_vec2()))
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

    fn revealed(&mut self, doc: &mut Document, out: &Rects, host: NodeId, main: f32) -> f32 {
        let Some(target) = doc.take_reveal(host) else {
            return 0.0;
        };
        let mut shown = Some(target);
        for _ in 0..crate::node::ANCESTOR_LIMIT {
            match shown {
                Some(node) if node == host => return 0.0,
                Some(node) if out.contains_key(&node) => break,
                Some(node) => shown = doc.arena.parent(node),
                None => return 0.0,
            }
        }
        let Some((node, start, rect)) = shown
            .and_then(|node| Some((node, self.content_start(out, host, node)?, out.get(&node)?)))
        else {
            return 0.0;
        };
        let extent = doc.outlined_extent(out, node).unwrap_or(rect);
        let end = start + self.direction.main(extent.max - rect.min);
        let start = start - self.direction.main(rect.min - extent.min);
        let Some(revealed) = revealed_offset(start, end, self.offset, main) else {
            return 0.0;
        };
        if !self.on_steer.is_empty() {
            let on_steer = self.on_steer;
            doc.report_after_layout(move || on_steer.call(()));
        }
        revealed - self.offset
    }

    fn anchor(&self, doc: &Document, out: &Rects, host: NodeId) -> Option<OffsetAnchor> {
        if !self.anchored {
            return None;
        }
        let mut anchor = self.child?;
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
const END_SLACK: f32 = 1.0;

fn revealed_offset(start: f32, end: f32, offset: f32, length: f32) -> Option<f32> {
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
        let size = self.child.map_or(Vec2::ZERO, |child| {
            crate::layout::measure(
                doc,
                painter,
                child,
                self.direction.axes(f32::INFINITY, cross),
            )
        });
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
        let snapping = self.snaps_to_end();
        let anchor = match snapping {
            true => None,
            false => self.anchor(doc, out, host),
        };
        self.anchored = true;
        let content = self.content_length(doc, painter, cross);
        if snapping {
            self.offset = self.end(main, content);
        }
        let position = self.settled(main, content);
        let outer = doc.take_anchor_shift();
        let invalidated = doc.invalidated_after_layout();
        let pinned = doc.replace_pinned_to_end(snapping.then_some(self.direction));
        self.place(doc, painter, rect, out, position, host);
        doc.replace_pinned_to_end(pinned);
        let settling = doc.invalidated_after_layout() != invalidated;
        let shift = doc.take_anchor_shift() + self.drift(doc, out, host, anchor);
        let revealed = self.revealed(doc, out, host, main);
        doc.report_anchor_shift(outer);
        let shift = match snapping {
            true => revealed,
            false => shift + revealed,
        };
        if shift != 0.0 {
            self.offset += shift;
            self.anchored = false;
            doc.invalidate_after_layout(host);
        }
        if snapping && revealed != 0.0 {
            self.to_end = false;
            self.pinned = self.offset >= position.max_offset() - END_SLACK;
        }
        if self.to_end && !settling {
            self.to_end = false;
        }
        self.position = Some(position);
        if !self.on_change.is_empty() && self.reported != Some(position) {
            self.reported = Some(position);
            let on_change = self.on_change;
            doc.report_after_layout(move || on_change.call(position));
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        let Some(host) = self.host else {
            return;
        };
        let Some(child) = self.child.filter(|child| rects.contains_key(child)) else {
            return;
        };
        let entered = painter.shifted(SpaceId::inside(host, 1), self.translation, rect);
        crate::paint::paint(doc, &entered, rects, child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn for_each_child(&self, visit: &mut dyn FnMut(NodeId)) {
        if let Some(child) = self.child {
            visit(child);
        }
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
    }

    fn kind(&self) -> &'static str {
        "offset"
    }

    fn detail(&self) -> Option<String> {
        (self.direction == Direction::Horizontal).then(|| "horizontal".to_string())
    }

    fn properties(&self) -> Vec<(&'static str, String)> {
        let mut properties = vec![
            ("direction", format!("{:?}", self.direction)),
            ("offset", self.offset.to_string()),
            ("fits", self.fits.to_string()),
        ];
        if self.follows_end {
            properties.push(("follows end", self.pinned.to_string()));
        }
        if let Some(position) = self.position {
            properties.push(("content", position.content.to_string()));
            properties.push(("viewport", position.viewport.to_string()));
        }
        properties
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
        let on_steer = node.on_steer;
        if node.offset != value || node.anchored {
            self.arena
                .relayout_alone_mut_as::<OffsetNode>(offset)
                .moved_to(value);
        }
        if on_steer.is_empty() {
            return;
        }
        match self.laying_out() {
            Some(_) => self.report_after_layout(move || on_steer.call(())),
            None => crate::current::enter(self, || on_steer.call(())),
        }
    }

    pub fn drive_offset(&mut self, offset: NodeOf<OffsetNode>, value: f32) {
        if self.arena.get_as::<OffsetNode>(offset).offset == value {
            return;
        }
        self.arena
            .relayout_alone_mut_as::<OffsetNode>(offset)
            .moved_to(value);
    }

    pub fn set_offset_follows_end(&mut self, offset: NodeOf<OffsetNode>, follows: bool) {
        if self.arena.get_as::<OffsetNode>(offset).follows_end != follows {
            self.arena
                .relayout_alone_mut_as::<OffsetNode>(offset)
                .follows_end = follows;
        }
    }

    pub fn offset_to_end(&mut self, offset: NodeOf<OffsetNode>) {
        let node = self.arena.relayout_alone_mut_as::<OffsetNode>(offset);
        node.pinned = true;
        node.to_end = true;
        node.anchored = false;
    }

    pub fn set_offset_on_steer(
        &mut self,
        offset: NodeOf<OffsetNode>,
        handler: impl FnMut(()) + 'static,
    ) {
        self.arena.touch_mut_as::<OffsetNode>(offset).on_steer = Callback::new(handler);
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
            let node = self.arena.get_as::<OffsetNode>(offset);
            node.position.map_or(0.0, |position| {
                ScrollPosition {
                    offset: node.offset,
                    ..position
                }
                .overscroll()
            })
        })
    }

    pub fn set_offset_on_change(
        &mut self,
        offset: NodeOf<OffsetNode>,
        handler: impl FnMut(ScrollPosition) + 'static,
    ) {
        self.arena.touch_mut_as::<OffsetNode>(offset).on_change = Callback::new(handler);
    }
}

impl Document {
    fn reveal_in(&mut self, host: NodeId, target: NodeId) {
        self.reveals.insert(host, target);
        if !self.arena.taken(host) {
            self.arena.invalidate_node(host);
        }
    }

    pub(crate) fn take_reveal(&mut self, offset: NodeId) -> Option<NodeId> {
        self.reveals.remove(&offset)
    }

    fn enclosing_viewers(&self, node: NodeId) -> Vec<NodeId> {
        let mut viewers = Vec::new();
        let mut current = self.arena.parent(node);
        for _ in 0..crate::node::ANCESTOR_LIMIT {
            let Some(id) = current else {
                break;
            };
            if self.arena.made_as::<OffsetNode>(id).is_some()
                || self.arena.made_as::<CanvasNode>(id).is_some()
            {
                viewers.push(id);
            }
            current = self.arena.parent(id);
        }
        viewers
    }

    pub fn reveal_node(&mut self, node: NodeId) {
        for viewer in self.enclosing_viewers(node) {
            self.reveal_in(viewer, node);
        }
    }

    pub(crate) fn reveal_in_canvases(&mut self, node: NodeId) {
        for viewer in self.enclosing_viewers(node) {
            if self.arena.made_as::<CanvasNode>(viewer).is_some() {
                self.reveal_in(viewer, node);
            }
        }
    }

    pub fn reveal_focus(&mut self) {
        if let Some(focused) = self.focused {
            self.reveal_node(focused);
        }
    }
}
