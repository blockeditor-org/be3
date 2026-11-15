use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(keys: Prop<Vec<u32>>) -> NodeId {
    let named = NodeRef::new();
    view! { <List spacing=Space::None><ForEach @node_ref=&named value={keys}>|_key: u32| <Spacer /></ForEach></List> } //~ ERROR is no node, so `@test_id` and `@node_ref` have nothing to name
}
