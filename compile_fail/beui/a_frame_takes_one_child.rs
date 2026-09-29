use beui::NodeId;
use beui::reactive::{Frame, Spacer, component, view};

#[component]
fn Case() -> NodeId {
    view! { <Frame><Spacer /><Spacer /></Frame> } //~ ERROR this component builds at most one child
}
