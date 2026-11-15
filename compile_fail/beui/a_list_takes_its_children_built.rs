use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! { <List spacing=Space::None>|| { view! { <Spacer /> } }</List> } //~ ERROR this component takes its children already built
}

#[component]
fn Named() -> NodeId {
    view! { <List spacing=Space::None>|item| { view! { <Spacer /> } }</List> } //~ ERROR this component takes its children already built
}
