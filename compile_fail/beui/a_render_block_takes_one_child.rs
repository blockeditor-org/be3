use beui::NodeId;
use beui::reactive::*;
use beui::styled::*;

#[component]
fn Case(outer: Prop<bool>, inner: Prop<bool>) -> NodeId {
    view! { <List spacing=0.0><Dynamic value={outer}>|_: bool|<Show condition={inner}><List spacing=0.0 /></Show></Dynamic></List> } //~ ERROR builds a run of children, and this slot takes exactly one child
}
