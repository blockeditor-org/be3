use beui_macros::{component, view};

use crate::theme::NARROW_WIDTH;
use beui_components_unstyled as unstyled;
use beui_components_unstyled::narrower_than;
use beui_core::node::NodeId;
use beui_view::reactive::{Children, ListChild, Prop};

#[component]
pub fn Stack(
    spacing: Prop<f32>,
    #[prop(default = NARROW_WIDTH)] breakpoint: f32,
    children: Children<ListChild>,
) -> NodeId {
    let narrow = narrower_than(breakpoint);
    view! {
        <unstyled::Stack spacing narrow children />
    }
}
