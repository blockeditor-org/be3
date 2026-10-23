use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn removing_a_keyed_node_drops_the_test_ids_it_registered() {
    let _root = crate::reactive::RootScope::new();
    let drop_two = NodeRef::new();
    let document = build_node(move || {
        let items = create_signal(vec![1i64, 2]);
        view! {
            <List spacing=0.0>
                <Button @node_ref=&drop_two on_click={move || items.set(vec![1])}>
                    <Text>"drop"</Text>
                </Button>
                <ForEach keys={items}>|value: i64|
                    <Text @test_id={format!("item.{value}")}>{value.to_string()}</Text>
                </ForEach>
            </List>
        }
    });

    let drop_two = drop_two.get();
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert!(harness.document().find_test_id("item.1").is_some());
    assert!(harness.document().find_test_id("item.2").is_some());

    harness.click(harness.center(drop_two));
    harness.frame(Vec::new());

    assert!(
        harness.document().find_test_id("item.1").is_some(),
        "a key that survives must keep its test id"
    );
    assert_eq!(
        harness.document().find_test_id("item.2"),
        None,
        "a removed keyed node must not leave its test id behind"
    );
}
