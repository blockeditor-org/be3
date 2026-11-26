use std::any::Any;

use crate::document::Document;
use crate::node::{Element, NodeOf, Rects};
use crate::painter::Painter;
use beui_types::NodeId;
use beui_types::{Rect, Vec2};

/// Draws another node, `of`, where the replica is, instead of where `of` is laid out: `of`
/// keeps its place in layout and its input, and draws nothing there while it is lent
/// (`Document::set_replica_of`), so no clip around it cuts what the replica draws. The node
/// is drawn by one painter at a time, so its retained paint and damage stay matched by
/// identity: its changes mark the replica (`PaintCache::lend`), which draws them where it is.
pub struct ReplicaNode {
    pub of: Option<NodeId>,
}

impl Element for ReplicaNode {
    fn measure(&self, _doc: &mut Document, _painter: &Painter, available: Vec2) -> Vec2 {
        let fill = |length: f32| match length.is_finite() {
            true => length,
            false => 0.0,
        };
        Vec2::new(fill(available.x), fill(available.y))
    }

    fn layout(&mut self, _doc: &mut Document, _painter: &Painter, _rect: Rect, _out: &Rects) {}

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        let Some(of) = self.of else {
            return;
        };
        let Some(placed) = rects.placed(&of) else {
            return;
        };
        // Drawn from this node's corner, as it is laid out where it is.
        let shifted = painter.transformed(-placed.rect.min.to_vec2(), 1.0);
        doc.replicating.set(Some(of));
        crate::paint::paint(doc, &shifted, rects, of);
        doc.replicating.set(None);
    }

    fn children(&self) -> Vec<NodeId> {
        Vec::new()
    }

    fn remove_child(&mut self, _child: NodeId) {}

    fn kind(&self) -> &'static str {
        "replica"
    }

    fn detail(&self) -> Option<String> {
        self.of.map(|of| format!("{of:?}"))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_replica(&mut self) -> NodeOf<ReplicaNode> {
        self.arena.insert(ReplicaNode { of: None })
    }

    /// Makes `replica` draw `of` (`None` draws nothing): `of` is lent to it and its parent
    /// draws it no more, until it is given back here or the replica goes.
    pub fn set_replica_of(&mut self, replica: NodeOf<ReplicaNode>, of: Option<NodeId>) {
        if !self.contains(replica) {
            return;
        }
        let held = self.arena.get_as::<ReplicaNode>(replica).of;
        if held == of {
            return;
        }
        let cache = self.paint_cache.get_mut();
        if let Some(held) = held
            && cache.replica_of(held) == Some(replica.id())
        {
            cache.lend(held, None);
            self.give_back(held);
        }
        if let Some(of) = of {
            self.paint_cache.get_mut().lend(of, Some(replica.id()));
            self.give_back(of);
        }
        self.arena.paint_mut_as::<ReplicaNode>(replica).of = of;
    }

    /// Repaints `of` and its parent, which draws it again or no more.
    fn give_back(&mut self, of: NodeId) {
        if !self.arena.contains(of) {
            return;
        }
        if let Some(parent) = self.arena.parent(of) {
            self.arena.repaint_node(parent);
        }
        self.arena.repaint_node(of);
    }
}
