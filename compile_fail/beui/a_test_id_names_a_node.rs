use beui::NodeId;
use beui::reactive::{List, Prop, Show, Spacer, component, view};

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <List spacing=0.0><Show @test_id="case" condition={shown}><Spacer /></Show></List> } //~ ERROR is no node, so `@test_id` and `@node_ref` have nothing to name
}
