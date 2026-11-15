use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(count: Signal<u32>) -> NodeId {
    view! { <List spacing=Space::None><Keyed value={count} key={|count: u32| count} fallback=()>|_: ReadSignal<u32>| <Frame /></Keyed></List> } //~ ERROR so a value is not lifted into an `Option`
}
