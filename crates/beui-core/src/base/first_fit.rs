use std::any::Any;

use crate::child_list::{ChildHost, ChildItem, ChildList, NodeChildren};
use crate::document::Document;
use crate::geometry::{Rect, Vec2, vec2};
use crate::node::{Element, FocusChild, NodeId, NodeOf, Rects};
use crate::painter::Painter;

const SLACK: f32 = 0.01;

#[derive(Default)]
pub struct FirstFitNode {
    items: ChildList<NodeId>,
    chosen: Option<NodeId>,
}

impl ChildHost for FirstFitNode {
    type Stored = NodeId;

    fn children(&mut self) -> &mut ChildList<NodeId> {
        &mut self.items
    }

    fn children_changed(&mut self) {
        self.chosen = None;
    }
}

impl FirstFitNode {
    fn choose(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<NodeId> {
        let last = self.items.iter().last().copied();
        if !available.x.is_finite() {
            return self.items.iter().next().copied();
        }
        self.items
            .iter()
            .copied()
            .filter(|item| Some(*item) != last)
            .find(|item| {
                let natural =
                    crate::layout::measure(doc, painter, *item, vec2(f32::INFINITY, available.y));
                natural.x <= available.x + SLACK
            })
            .or(last)
    }
}

impl Element for FirstFitNode {
    fn paints_from_layout(&self) -> bool {
        false
    }

    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.choose(doc, painter, available)
            .map_or(Vec2::ZERO, |item| {
                crate::layout::measure(doc, painter, item, available)
            })
    }

    fn min_width(&self, doc: &mut Document, painter: &Painter) -> f32 {
        self.items
            .iter()
            .last()
            .map_or(0.0, |item| crate::layout::min_width(doc, painter, *item))
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        let item = self.choose(doc, painter, available)?;
        crate::layout::baseline(doc, painter, item, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        self.chosen = self.choose(doc, painter, rect.size());
        if let Some(item) = self.chosen {
            crate::layout::layout(doc, painter, item, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        if let Some(item) = self.chosen {
            crate::paint::paint(doc, painter, rects, item);
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

    fn focus_children(&self) -> Option<Vec<FocusChild>> {
        let shown = self.chosen.or_else(|| self.items.iter().next().copied());
        Some(shown.into_iter().map(FocusChild::Node).collect())
    }

    fn remove_child(&mut self, child: NodeId) {
        crate::child_list::remove_hosted(self, child);
    }

    fn kind(&self) -> &'static str {
        "first fit"
    }

    fn detail(&self) -> Option<String> {
        let chosen = self.chosen?;
        let index = self.items.iter().position(|item| *item == chosen)?;
        Some(format!("showing child {}", index + 1))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_first_fit(&mut self) -> NodeOf<FirstFitNode> {
        self.arena.insert(FirstFitNode::default())
    }
}
