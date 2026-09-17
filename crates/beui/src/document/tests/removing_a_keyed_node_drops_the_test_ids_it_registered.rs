use super::*;
use crate::reactive::{Button, Column, ForEach, NodeRef, Text, build, create_signal, view};

#[test]
fn removing_a_keyed_node_drops_the_test_ids_it_registered() {
    let drop_two = NodeRef::new();
    let document = build({
        let drop_two = drop_two.clone();
        move || {
            let (items, set_items) = create_signal(vec![1i64, 2]);
            view! {
                <Column spacing=0.0>
                    <Button @node_ref=&drop_two on_click={move || set_items.set(vec![1])}>
                        <Text string="drop" />
                    </Button>
                    <ForEach spacing=0.0 keys=items>
                        {|value: i64| view! {
                            <Text @test_id={format!("item.{value}")} string={value.to_string()} />
                        }}
                    </ForEach>
                </Column>
            }
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
