use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Signal<bool>) -> NodeId {
    view! { <Frame opacity={move || shown.get()} /> } //~ ERROR cannot be passed as a `f32` prop
}
