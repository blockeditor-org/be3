use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <Card><Show condition={shown}><Frame /></Show></Card> } //~ ERROR can build `ZeroOrOne`
}
