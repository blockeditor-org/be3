use std::any::Any;

use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub struct ShiftNode {
    child: NodeId,
    by: Vec2,
}

impl Element for ShiftNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        crate::layout::measure(doc, painter, self.child, available)
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let rect = rect.translate(doc.pixel_grid().snap_vec(self.by));
        crate::layout::layout(doc, painter, self.child, rect, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        let viewport = doc.viewport_rect().translate(-painter.origin());
        if self.by != Vec2::ZERO
            && rects
                .get(&self.child)
                .is_some_and(|placed| !placed.intersects(viewport))
        {
            return;
        }
        crate::paint::paint(doc, painter, rects, self.child);
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
        "shift"
    }

    fn detail(&self) -> Option<String> {
        (self.by != Vec2::ZERO).then(|| format!("{:.1}, {:.1}", self.by.x, self.by.y))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_shift(&mut self, child: NodeId) -> NodeOf<ShiftNode> {
        self.arena.insert(ShiftNode {
            child,
            by: Vec2::ZERO,
        })
    }

    pub fn set_shift(&mut self, shift: NodeOf<ShiftNode>, by: Vec2) {
        if self.arena.get_as::<ShiftNode>(shift).by != by {
            self.arena.get_mut_as::<ShiftNode>(shift).by = by;
        }
    }
}
