use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(keys: Prop<Vec<u32>>, shown: Prop<bool>) -> NodeId {
    view! { <List spacing=0.0><ForEach keys>|_key: u32| <Show condition={shown.clone()}><List spacing=0.0 /></Show></ForEach></List> } //~ ERROR builds a run of children, and this slot takes exactly one child
}
