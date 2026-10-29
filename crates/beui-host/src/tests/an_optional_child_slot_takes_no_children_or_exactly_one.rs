use super::*;
use crate::primitives::*;

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
    let mut harness = Harness::new(document);

    harness.toggle_inspector();

    assert_eq!(harness.tree(), ["list", "  frame", "  frame", "    text"]);
}
