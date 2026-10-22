use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[component]
fn Widget() -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=0.0></List>
    }
}

#[test]
fn a_component_function_returns_its_base_node() {
    let document = build_node(|| {
        view! {
            <List spacing=0.0>
                <Widget />
            </List>
        }
    });
    let mut harness = harness(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["list", "  list"]);
}
