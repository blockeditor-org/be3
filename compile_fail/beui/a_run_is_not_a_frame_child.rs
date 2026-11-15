use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <Frame><Show condition={shown} fallback={|| view! { <Frame /> }}>|| <Frame /><Frame /></Show></Frame> } //~ ERROR can build `Many`
}
