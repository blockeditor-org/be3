use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(outer: Prop<bool>, inner: Prop<bool>) -> NodeId {
    view! { <Card><Dynamic value={outer}>|_: bool| <Show condition={inner}><Frame /></Show></Dynamic></Card> } //~ ERROR can build `ZeroOrOne` children
}
