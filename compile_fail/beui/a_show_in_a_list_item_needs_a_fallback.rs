use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <List spacing=Space::None><ListItem grow=1.0><Show condition={shown}><Frame /></Show></ListItem></List> } //~ ERROR add `fallback=…` so the slot always holds a child
}
