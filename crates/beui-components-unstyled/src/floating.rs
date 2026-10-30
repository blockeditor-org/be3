use beui_macros::{component, view};
use beui_view::types::LayerPolicy;

use beui_view::components::overlay::Overlay;
use beui_view::reactive::NodeId;
use beui_view::reactive::*;
use beui_view::types::Placement;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Edge {
    Top,
    TopStart,
    TopEnd,
    #[default]
    Bottom,
    BottomStart,
    BottomEnd,
}

#[component]
pub fn Floating(
    anchor: NodeRef,
    #[prop(default = Edge::Bottom)] edge: Prop<Edge>,
    #[prop(default = true)] open: Prop<bool>,
    #[prop(default = true)] interactive: Prop<bool>,
    #[prop(default = false)] keep_alive: bool,
    #[prop(children)] content: RenderFn<()>,
) -> NodeId {
    let placement = edge.map(|edge| match edge {
        Edge::Top => Placement::InsideTop,
        Edge::TopStart => Placement::InsideTopStart,
        Edge::TopEnd => Placement::InsideTopEnd,
        Edge::Bottom => Placement::InsideBottom,
        Edge::BottomStart => Placement::InsideBottomStart,
        Edge::BottomEnd => Placement::InsideBottomEnd,
    });
    let mode = interactive.map(|interactive| match interactive {
        true => LayerPolicy::Pass,
        false => LayerPolicy::Inert,
    });
    view! {
        <Overlay
            anchor=anchor
            placement={placement}
            policy={mode}
            traps_focus=false
            open={open}
            keep_alive
            content={move || content.call(())}
        />
    }
}
