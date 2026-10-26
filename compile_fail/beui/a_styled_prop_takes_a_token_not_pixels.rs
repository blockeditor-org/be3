use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! { <List spacing=8.0><Spacer /></List> } //~ ERROR beui_components_styled::Space>` is not satisfied
}
