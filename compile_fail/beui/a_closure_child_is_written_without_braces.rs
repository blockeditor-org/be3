use beui::NodeId;
use beui::reactive::*;

#[component]
fn Case(keys: Prop<Vec<u32>>) -> NodeId {
    view! { <List spacing=Space::None><ForEach keys>{move |_key: u32| view! { <Spacer /> }}</ForEach></List> } //~ ERROR write a closure child without its braces and `move`
}
