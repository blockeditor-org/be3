use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn a_reactive_test_id_follows_its_signal() {
    let _root = crate::reactive::RootScope::new();
    let rename = NodeRef::new();
    let document = build_node(move || {
        let row = create_signal(0usize);
        let test_id = create_memo(move || format!("cell.{}", row.get()));
        view! {
            <List spacing=0.0>
                <Button @node_ref=&rename on_click={move || row.set(1)}>
                    <Text>"rename"</Text>
                </Button>
                <Text @crate::reactive::test_id={test_id}>"value"</Text>
            </List>
        }
    });

    let rename = rename.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let named = harness.document().find_test_id("cell.0");
    assert!(named.is_some(), "the first name must reach the node");

    harness.click(harness.center(rename));
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().find_test_id("cell.0"),
        None,
        "the name the signal left behind must not stay registered"
    );
    assert_eq!(
        harness.document().find_test_id("cell.1"),
        named,
        "the new name must reach the same node"
    );
}
