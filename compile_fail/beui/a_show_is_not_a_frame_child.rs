use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <Frame><Show condition={shown}><Spacer /></Show></Frame> } //~ ERROR builds a run of children, and this slot takes exactly one child
}
