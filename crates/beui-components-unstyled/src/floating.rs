use beui_core::layer::LayerPolicy;
use beui_macros::{component, view};

use beui_core::base::overlay::Placement;
use beui_core::node::NodeId;
use beui_view::components::overlay::Overlay;
use beui_view::reactive::*;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Edge {
    Top,
    TopEnd,
    #[default]
    Bottom,
    BottomEnd,
}

#[component]
pub fn Floating(
    anchor: NodeRef,
    #[prop(default = Edge::Bottom)] edge: Prop<Edge>,
    #[prop(default = true)] open: Prop<bool>,
    #[prop(default = true)] interactive: Prop<bool>,
    children: Child,
) -> NodeId {
    let placement = edge.map(|edge| match edge {
        Edge::Top => Placement::InsideTop,
        Edge::TopEnd => Placement::InsideTopEnd,
        Edge::Bottom => Placement::InsideBottom,
        Edge::BottomEnd => Placement::InsideBottomEnd,
    });
    let mode = interactive.map(|interactive| match interactive {
        true => LayerPolicy::Pass,
        false => LayerPolicy::Inert,
    });
    view! {
        <Overlay anchor=anchor placement={placement} policy={mode} traps_focus=false open={open}>
            {children}
        </Overlay>
    }
}
