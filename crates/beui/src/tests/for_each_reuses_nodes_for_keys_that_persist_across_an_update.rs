use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn for_each_reuses_nodes_for_keys_that_persist_across_an_update() {
    let _root = crate::reactive::RootScope::new();
    let (list, shuffle) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        let items = create_signal(vec![1i64, 2, 3]);
        view! {
            <List spacing=0.0>
                <Button @node_ref=&shuffle on_click={move || items.set(vec![3, 2, 4])}>
                    <Text>"shuffle"</Text>
                </Button>
                <List @node_ref=&list spacing=0.0>
                    <ForEach keys={items}>|value: i64|
                        <Text>{value.to_string()}</Text>
                    </ForEach>
                </List>
            </List>
        }
    });

    let (list, shuffle) = (list.get(), shuffle.get());
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let before = harness.document().children(list);
    assert_eq!(
        before
            .iter()
            .map(|&id| text_of(harness.document(), id).to_owned())
            .collect::<Vec<_>>(),
        vec!["1", "2", "3"]
    );
    let node_for_two = before[1];
    let node_for_three = before[2];

    harness.click(harness.center(shuffle));
    harness.frame(Vec::new());

    let after = harness.document().children(list);
    assert_eq!(
        after
            .iter()
            .map(|&id| text_of(harness.document(), id).to_owned())
            .collect::<Vec<_>>(),
        vec!["3", "2", "4"],
        "surviving and new items must appear in the new order"
    );
    assert_eq!(
        after[0], node_for_three,
        "a key that persists must keep its node identity"
    );
    assert_eq!(
        after[1], node_for_two,
        "a key that persists must keep its node identity"
    );
    assert!(
        !before.contains(&after[2]),
        "a new key must get a freshly built node"
    );
}
