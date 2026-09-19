use beui_macros::{component, view};

use crate::base::overlay::{Overlay, OverlayMode, Placement};
use crate::node::NodeId;
use crate::reactive::{Child, NodeRef, Prop};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Edge {
    Top,
    #[default]
    Bottom,
}

#[component]
pub fn Floating(
    anchor: NodeRef,
    #[prop(default = Edge::Bottom)] edge: Prop<Edge>,
    #[prop(default = true)] open: Prop<bool>,
    children: Child,
) -> NodeId {
    let placement = edge.map(|edge| match edge {
        Edge::Top => Placement::InsideTop,
        Edge::Bottom => Placement::InsideBottom,
    });
    view! {
        <Overlay
            anchor=&anchor
            placement={placement}
            mode=OverlayMode::Floating
            traps_focus=false
            open={open}
        >
            {children}
        </Overlay>
    }
}
