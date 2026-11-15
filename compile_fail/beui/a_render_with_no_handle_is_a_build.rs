use beui::NodeId;
use beui::reactive::*;

#[component]
fn Case(#[prop(children)] content: Render<(), NodeId>) -> NodeId { //~ ERROR a render with no handle is a `Build<C>`
    content.call(())
}
