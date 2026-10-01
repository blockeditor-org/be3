use std::any::Any;

use crate::base::child_list::{ChildHost, ChildItem, ChildList};
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, InteractInput, NodeId, Rects, NodeOf};
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
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.items.iter().fold(Vec2::ZERO, |size, item| {
            size.max(crate::layout::measure(doc, painter, *item, available))
        })
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
        children.extend(self.items.iter().map(ChildItem::node));
    }

    fn children(&self) -> Vec<NodeId> {
        self.items.nodes()
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
