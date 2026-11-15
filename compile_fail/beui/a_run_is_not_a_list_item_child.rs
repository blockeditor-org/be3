use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(shown: Prop<bool>) -> NodeId {
    view! { <List spacing=Space::None><ListItem grow=1.0><Show condition={shown}>|| <Frame /><Frame /></Show></ListItem></List> } //~ ERROR can build `beui::reactive::Many` children
}
