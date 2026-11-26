use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! { <Frame><Frame /><Frame /></Frame> } //~ ERROR this component builds at most one child
    //~ NOTE required for `beui::NodeId` to implement `TakesOptionalBlock<
    //~ NOTE required for `Option<beui::NodeId>` to implement `TakesBlock<
}
