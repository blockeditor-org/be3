use super::*;
use crate::primitives::Spacer;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_tag_can_take_a_node_ref_and_a_test_id_slot_at_once() {
    let _root = crate::reactive::RootScope::new();
    let spacer = NodeRef::new();
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Spacer @node_ref=&spacer @test_id="column.spacer" />
            </List>
        }
    });

    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(harness.find("column.spacer"), spacer.get());
}
