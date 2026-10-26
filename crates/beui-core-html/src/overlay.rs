use beui_tree::reactive::{IntoProp, Prop};
use beui_types::{Pos2, Rect};

use crate::node::NodeRef;

#[derive(Clone, PartialEq)]
pub enum OverlayAnchor {
    Node(NodeRef),
    Point(Pos2),
    Rect(Rect),
    Pinned { node: NodeRef, rect: Rect },
}

impl IntoProp<OverlayAnchor> for &NodeRef {
    fn into_prop(self) -> Prop<OverlayAnchor> {
        Prop::Static(OverlayAnchor::Node(*self))
    }
}

impl IntoProp<OverlayAnchor> for NodeRef {
    fn into_prop(self) -> Prop<OverlayAnchor> {
        Prop::Static(OverlayAnchor::Node(self))
    }
}

impl Default for OverlayAnchor {
    fn default() -> Self {
        OverlayAnchor::Point(Pos2::ZERO)
    }
}
