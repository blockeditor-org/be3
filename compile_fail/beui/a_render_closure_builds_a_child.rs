use beui::NodeId;
use beui::reactive::*;

#[component]
fn Titled(header: RenderFn<u32, NodeId>) -> NodeId {
    header.call(1)
}

#[component]
fn Case() -> NodeId {
    view! { <Titled header={|_: u32| 5_u32} /> } //~ ERROR a `u32` cannot be a child of this component
}
