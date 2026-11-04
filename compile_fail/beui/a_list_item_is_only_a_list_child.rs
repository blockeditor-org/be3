use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! { <Frame><ListItem grow=1.0><Frame /></ListItem></Frame> } //~ ERROR a child made for a `ListKind` cannot be a child of this component
}
