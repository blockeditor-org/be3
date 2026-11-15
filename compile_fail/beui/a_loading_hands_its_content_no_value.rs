use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! { <List spacing=Space::None><Loading fallback={|_: Pending| view! { <Spacer /> }}>|value| <Spacer /></Loading></List> } //~ ERROR this component hands nothing to its children, so there is no value to name
}
