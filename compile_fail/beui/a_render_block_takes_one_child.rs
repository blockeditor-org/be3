use beui::NodeId;
use beui::reactive::{List, Prop, Show, component, view};

#[component]
fn Case(outer: Prop<bool>, inner: Prop<bool>) -> NodeId {
    view! { <List spacing=0.0><Show condition={outer}><Show condition={inner}><List spacing=0.0 /></Show></Show></List> } //~ ERROR builds a run of children, and this slot takes exactly one child
}
