use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <List spacing=0.0><Show condition={shown}></Show></List> } //~ ERROR this component builds exactly one child
}
