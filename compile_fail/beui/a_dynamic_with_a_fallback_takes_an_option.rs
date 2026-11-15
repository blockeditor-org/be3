use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(count: Signal<u32>) -> NodeId {
    view! { <Card><Dynamic value={count} fallback={|| view! { <Frame /> }}>|_: u32| <Frame /></Dynamic></Card> } //~ ERROR so a value is not lifted into an `Option`
}
