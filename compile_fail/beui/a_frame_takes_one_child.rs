use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! { <Frame><Frame /><Frame /></Frame> } //~ ERROR this component builds at most one child
}
