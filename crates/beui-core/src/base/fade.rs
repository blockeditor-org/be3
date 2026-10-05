use std::any::Any;

use crate::base::frame::Sides;
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, InteractInput, NodeId, NodeOf, Rects};
use crate::painter::Painter;

pub struct FadeNode {
    child: NodeId,
    edges: Sides,
}

impl Element for FadeNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        crate::layout::measure(doc, painter, self.child, available)
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        crate::layout::layout(doc, painter, self.child, rect, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        let Sides {
            left,
            top,
            right,
            bottom,
        } = self.edges;
        let faded = painter.faded(rect, [left, top, right, bottom]);
        crate::paint::paint(doc, &faded, rects, self.child);
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

    fn passes_scroll_anchor(&self) -> bool {
        true
    }

    fn kind(&self) -> &'static str {
        "fade"
    }

    fn detail(&self) -> Option<String> {
        let Sides {
            left,
            top,
            right,
            bottom,
        } = self.edges;
        (self.edges != Sides::default())
            .then(|| format!("{left:.1}, {top:.1}, {right:.1}, {bottom:.1}"))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_fade(&mut self, child: NodeId) -> NodeOf<FadeNode> {
        self.arena.insert(FadeNode {
            child,
            edges: Sides::default(),
        })
    }

    pub fn set_fade(&mut self, fade: NodeOf<FadeNode>, edges: Sides) {
        if self.delivering() {
            self.deferred_fades.push((fade, edges));
            return;
        }
        if self.arena.get_as::<FadeNode>(fade).edges != edges {
            self.arena.paint_mut_as::<FadeNode>(fade).edges = edges;
        }
    }
}
