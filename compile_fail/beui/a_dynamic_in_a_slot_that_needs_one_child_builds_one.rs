use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(outer: Prop<bool>, inner: Prop<bool>) -> NodeId {
    view! { <Card><Dynamic value={outer}>|_: bool| <Show value={inner}><Frame /></Show></Dynamic></Card> } //~ ERROR add `fallback=…` so the slot always holds a child
}
