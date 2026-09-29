use beui::NodeId;
use beui::reactive::{List, Prop, Show, component, view};

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <List spacing=0.0><Show condition={shown}></Show></List> } //~ ERROR this component builds exactly one child
}
