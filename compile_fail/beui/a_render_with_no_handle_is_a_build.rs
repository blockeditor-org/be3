use beui::reactive::*;

#[component]
fn Case(#[prop(children)] content: Render<(), beui::NodeId>) -> beui::NodeId { //~ ERROR a render with no handle is a `Build<C>`
    content.call(())
}
