use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(keys: Prop<Vec<u32>>) -> NodeId {
    view! { <Frame><ForEach value={keys}>|_key: u32| <Frame /></ForEach></Frame> } //~ ERROR put it in a `<List>` or `<Layers>`
}
