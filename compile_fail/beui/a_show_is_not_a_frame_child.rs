use beui::NodeId;
use beui::reactive::{Frame, Prop, Show, Spacer, component, view};

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <Frame><Show condition={shown}><Spacer /></Show></Frame> } //~ ERROR builds a run of children, and this slot takes exactly one child
}
