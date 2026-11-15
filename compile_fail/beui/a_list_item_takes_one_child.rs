use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! { <List spacing=Space::None><ListItem grow=1.0><Frame /><Frame /></ListItem></List> } //~ ERROR this component builds exactly one child
}
