use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <List spacing=Space::None><Show @test_id="case" condition={shown}><Spacer /></Show></List> } //~ ERROR is no node, so `@test_id` has nothing to name
}
