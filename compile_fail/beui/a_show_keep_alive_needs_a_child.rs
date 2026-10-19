use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <List spacing=0.0><ShowKeepAlive condition={shown}></ShowKeepAlive></List> } //~ ERROR this component builds exactly one child
}
