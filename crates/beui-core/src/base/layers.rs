use std::any::Any;

use crate::base::list::Direction;
use crate::child_list::{ChildHost, ChildItem, ChildList, NodeChildren};
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, NodeId, NodeOf, Rects};
use crate::painter::Painter;

#[derive(Default)]
pub struct LayersNode {
    items: ChildList<NodeId>,
}

impl ChildHost for LayersNode {
    type Stored = NodeId;

    fn children(&mut self) -> &mut ChildList<NodeId> {
        &mut self.items
    }
}

impl Element for LayersNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.items.iter().fold(Vec2::ZERO, |size, item| {
            size.max(crate::layout::measure(doc, painter, *item, available))
        })
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        self.items.iter().fold(0.0, |width, item| {
            width.max(crate::layout::min_width(doc, painter, *item))
        })
    }

    fn least_length(
        &self,
        doc: &mut Document,
        painter: &Painter,
        direction: Direction,
        cross: f32,
    ) -> Option<f32> {
        Some(self.items.iter().fold(0.0, |length, item| {
            length.max(crate::layout::least_length(
                doc, painter, *item, direction, cross,
            ))
        }))
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        let first = *self.items.iter().next()?;
        crate::layout::baseline(doc, painter, first, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        for item in self.items.iter() {
            crate::layout::layout(doc, painter, *item, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        for item in self.items.iter() {
            crate::paint::paint(doc, painter, rects, *item);
        }
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.nodes()
    }

    fn children_revision(&self) -> Option<u64> {
        Some(self.items.revision())
    }

    fn for_each_child(&self, visit: &mut dyn FnMut(NodeId)) {
        for child in self.items.iter().filter_map(ChildItem::node) {
            visit(child);
        }
    }

    fn remove_child(&mut self, child: NodeId) {
        crate::child_list::remove_hosted(self, child);
    }

    fn kind(&self) -> &'static str {
        "layers"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_layers(&mut self) -> NodeOf<LayersNode> {
        self.arena.insert(LayersNode::default())
    }
}
