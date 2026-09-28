use super::*;
use crate::reactive::{Direction, List, build, component, view};

#[component]
fn Widget() -> NodeId {
    view! {
        <List direction=Direction::Horizontal spacing=0.0></List>
    }
}

#[test]
fn a_component_function_returns_its_base_node() {
    let document = build(|| {
        view! {
            <List spacing=0.0>
                <Widget />
            </List>
        }
    });
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["column", "  row"]);
}
