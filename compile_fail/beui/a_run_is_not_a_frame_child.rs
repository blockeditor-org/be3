use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <Frame><Show value={shown} fallback={|| view! { <Frame /> }}>|| <Frame /><Frame /></Show></Frame> } //~ ERROR put it in a `<List>` or `<Layers>`
}
