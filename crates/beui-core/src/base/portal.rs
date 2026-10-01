use std::any::Any;

use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, InteractInput, NodeId, Rects};
use crate::painter::Painter;

pub struct PortalNode {
    pub child: Option<NodeId>,
}

impl Element for PortalNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn baseline(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Option<f32> {
        crate::layout::baseline(doc, painter, self.child?, available)
    }

    fn layout(&mut self, doc: &mut Document, painter: &Painter, rect: Rect, out: &Rects) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &Rects, _rect: Rect) {
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn paints(&self) -> bool {
        false
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
        children.extend(self.child);
    }

    fn children(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn borrowed(&self) -> Vec<NodeId> {
        self.child.into_iter().collect()
    }

    fn kind(&self) -> &'static str {
        "portal"
    }

    fn detail(&self) -> Option<String> {
        self.child.map(|child| format!("{child:?}"))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Document {
    pub fn create_portal(&mut self) -> NodeId {
        self.arena.insert(PortalNode { child: None })
    }

    pub fn set_portal_child(&mut self, portal: NodeId, child: Option<NodeId>) {
        if !self.contains(portal) {
            return;
        }
        let held = self.arena.get_as::<PortalNode>(portal).child;
        if held == child {
            return;
        }
        if let Some(held) = held
            && self.portal_holders.get(&held) == Some(&portal)
        {
            self.portal_holders.remove(&held);
        }
        if let Some(child) = child {
            if let Some(shown) = self.portal_holders.insert(child, portal)
                && shown != portal
                && self.contains(shown)
            {
                self.arena.get_mut_as::<PortalNode>(shown).child = None;
            }
            self.arena.get_mut_as::<PortalNode>(portal).child = Some(child);
            return;
        }
        self.arena.get_mut_as::<PortalNode>(portal).child = None;
    }

    pub fn release_portal(&mut self, id: NodeId, borrowed: &[NodeId]) {
        for child in borrowed {
            if self.portal_holders.get(child) == Some(&id) {
                self.portal_holders.remove(child);
            }
        }
        if let Some(portal) = self.portal_holders.remove(&id)
            && self.contains(portal)
        {
            self.arena.get_mut_as::<PortalNode>(portal).child = None;
        }
    }
}
