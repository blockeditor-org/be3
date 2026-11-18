use beui::NodeId;
use beui::reactive::*;

#[component]
fn Case(numbers: Signal<Vec<u32>>) -> NodeId {
    view! { <List spacing=Space::None><ForEach value={numbers}>|number: u32, index| <Spacer /></ForEach></List> } //~ ERROR give every parameter of this closure child its type, or none of them
}
