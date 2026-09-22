use std::any::Any;

use beui_macros::component;

use crate::document::Document;
use crate::geometry::{Rect, Vec2};
use crate::node::{Element, NodeId, NodeMap};
use crate::painter::Painter;
use crate::reactive::{Prop, create_effect, on_cleanup, with_document};

pub(crate) struct PortalNode {
    pub(crate) child: Option<NodeId>,
}

impl Element for PortalNode {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        match self.child {
            Some(child) => crate::layout::measure(doc, painter, child, available),
            None => Vec2::ZERO,
        }
    }

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut NodeMap<Rect>,
    ) {
        if let Some(child) = self.child {
            crate::layout::layout(doc, painter, child, rect, out);
        }
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &NodeMap<Rect>, _rect: Rect) {
        if let Some(child) = self.child {
            crate::paint::paint(doc, painter, rects, child);
        }
    }

    fn paints(&self) -> bool {
        false
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
    pub(crate) fn create_portal(&mut self) -> NodeId {
        self.arena.insert(PortalNode { child: None })
    }

    pub(crate) fn set_portal_child(&mut self, portal: NodeId, child: Option<NodeId>) {
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

    pub(crate) fn release_portal(&mut self, id: NodeId, borrowed: &[NodeId]) {
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

#[component]
pub fn Portal(#[prop(default = None)] node: Prop<Option<NodeId>>) -> NodeId {
    let portal = with_document(Document::create_portal);
    create_effect(move || {
        let node = node.get();
        with_document(|document| document.set_portal_child(portal, node));
    });
    on_cleanup(move || {
        crate::reactive::try_with_document(|document| document.set_portal_child(portal, None));
    });
    portal
}
