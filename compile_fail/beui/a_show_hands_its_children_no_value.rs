use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <List spacing=Space::None><Show condition={shown}>|value| <Spacer /></Show></List> } //~ ERROR this component hands nothing to its children, so there is no value to name
}
