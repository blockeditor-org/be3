use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_optional_child_slot_takes_no_children_or_exactly_one() {
    let document = build_node(|| {
        view! {
            <List spacing=0.0>
                <Frame></Frame>
                <Frame>
                    <Text>"only"</Text>
                </Frame>
            </List>
        }
    });
    let mut harness = harness(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["list", "  frame", "  frame", "    text"]);
}
