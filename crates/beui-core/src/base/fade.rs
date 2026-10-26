use std::any::Any;

use crate::base::frame::Sides;
use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, NodeId, NodeOf, Rects, SpaceId};
use crate::painter::Painter;

pub struct FadeNode {
    child: Option<NodeId>,
    edges: Sides,
    mask: Option<SpaceId>,
}

impl Element for FadeNode {
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
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, rect: Rect) {
        let Some(child) = self.child else {
            return;
        };
        let Sides {
            left,
            top,
            right,
            bottom,
        } = self.edges;
        let Some(mask) = self.mask else {
            return;
        };
        let faded = painter.faded(mask, rect, [left, top, right, bottom]);
        crate::paint::paint(doc, &faded, rects, child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn remove_child(&mut self, child: NodeId) {
        if self.child == Some(child) {
            self.child = None;
        }
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
        let fade = self.arena.insert(FadeNode {
            child: Some(child),
            edges: Sides::default(),
            mask: None,
        });
        let id = NodeId::from(fade);
        if let Some(node) = self
            .arena
            .state_mut(id)
            .as_any_mut()
            .downcast_mut::<FadeNode>()
        {
            node.mask = Some(SpaceId::of(id));
        }
        fade
    }

    pub fn set_fade(&mut self, fade: NodeOf<FadeNode>, edges: Sides) {
        if self.arena.get_as::<FadeNode>(fade).edges != edges {
            self.arena.paint_mut_as::<FadeNode>(fade).edges = edges;
        }
    }
}
