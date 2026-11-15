use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case() -> NodeId {
    view! { <Tabs options={view! { <ChoiceOption value=1 label="One" /> }} /> } //~ ERROR missing required prop `selected`
}
