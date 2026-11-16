use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! { <List spacing=Space::None><Spacer grow="wide" /></List> } //~ ERROR cannot be passed as an optional `f32` prop
}
