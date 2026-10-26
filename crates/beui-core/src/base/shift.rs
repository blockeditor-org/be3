use std::any::Any;

use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub struct ShiftNode {
    child: Option<NodeId>,
    by: Vec2,
}

impl Element for ShiftNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.child.map_or(Vec2::ZERO, |child| {
            crate::layout::measure(doc, painter, child, available)
        })
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        self.child
            .map_or(0.0, |child| crate::layout::min_width(doc, painter, child))
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child?, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        let rect = rect.translate(doc.pixel_grid().snap_vec(self.by));
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        let Some(child) = self.child else {
            return;
        };
        if self.by != Vec2::ZERO
            && rects
                .placed(&child)
                .is_some_and(|placed| !placed.rect.intersects(painter.clip_rect()))
        {
            return;
        }
        crate::paint::paint(doc, painter, rects, child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
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
            child: Some(child),
            by: Vec2::ZERO,
        })
    }

    pub fn set_shift(&mut self, shift: NodeOf<ShiftNode>, by: Vec2) {
        if self.arena.get_as::<ShiftNode>(shift).by != by {
            self.arena.get_mut_as::<ShiftNode>(shift).by = by;
        }
    }
}
