use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(keys: Prop<Vec<u32>>) -> NodeId {
    view! { <List spacing=Space::None><ForEach @test_id="case" keys>|_key: u32| <Spacer /></ForEach></List> } //~ ERROR is no node, so `@test_id` and `@node_ref` have nothing to name
}
