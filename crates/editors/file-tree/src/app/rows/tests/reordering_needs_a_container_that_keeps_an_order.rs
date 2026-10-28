use super::*;

#[test]
fn reordering_needs_a_container_that_keeps_an_order() {
    let client = client();
    let block_types = block_types();

    let unordered = catalog(ChildEdits {
        add: true,
        delete: true,
        replace: true,
        reorder: false,
    });
    assert!(!can_reorder_in(
        &client,
        &unordered,
        &block_types,
        Some(CONTAINER)
    ));

    let ordered = catalog(ChildEdits {
        reorder: true,
        ..ChildEdits::default()
    });
    assert!(
        !can_reorder_in(&client, &ordered, &block_types, None),
        "a block with no container in the tree has no siblings to move among"
    );
    assert!(can_reorder_in(
        &client,
        &ordered,
        &block_types,
        Some(CONTAINER)
    ));
}
