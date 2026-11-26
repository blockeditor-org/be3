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
    /// This node, whose space's scale `of` is drawn in proportion to.
    own: Option<NodeId>,
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
        // Drawn from this node's corner, at the scale `of` shows at (through a zoomed canvas
        // around it, say) as this node's space draws it. The rotation and opacity of `of`'s
        // ancestors are not drawn.
        let own = self.own.map_or(1.0, |own| rects.scale_of(&own));
        let scale = match rects.scale_of(&of) / own {
            scale if scale.is_finite() && scale > 0.0 => scale,
            _ => 1.0,
        };
        let shifted = painter.transformed(-placed.rect.min.to_vec2() * scale, scale);
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
        let replica = self.arena.insert(ReplicaNode {
            of: None,
            own: None,
        });
        self.arena.downcast_mut(replica).own = Some(replica.id());
        replica
    }

    /// Makes `replica` draw `of` (`None` draws nothing): `of` is lent to it and its parent
    /// draws it no more, until it is given back here or the replica goes. A node is lent to
    /// one replica at a time: lent again, the replica that held it draws nothing. A replica
    /// within `of` would draw itself without end, so it is refused.
    pub fn set_replica_of(&mut self, replica: NodeOf<ReplicaNode>, of: Option<NodeId>) {
        if !self.contains(replica) {
            return;
        }
        let of = of.filter(|&of| {
            let within = self.is_within(replica.id(), of);
            debug_assert!(!within, "a replica cannot draw a node it is within");
            !within
        });
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
            let cache = self.paint_cache.get_mut();
            if let Some(holder) = cache.replica_of(of)
                && self.arena.contains(holder)
                && let Some(holder_node) = self
                    .arena
                    .state_mut(holder)
                    .as_any_mut()
                    .downcast_mut::<ReplicaNode>()
            {
                holder_node.of = None;
                self.arena.repaint_node(holder);
            }
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
