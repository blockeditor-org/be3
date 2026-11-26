use super::*;

#[test]
fn deleting_needs_a_container_that_can_delete_children() {
    let client = client();
    let block_types = block_types();

    let refusing = catalog(ChildOperations::default());
    assert!(!can_move_out_of(
        &client,
        &refusing,
        |id| block_types.get(&id).copied(),
        BlockParent::Block(CONTAINER),
        LISTED,
        false
    ));
    assert!(
        can_move_out_of(
            &client,
            &refusing,
            |id| block_types.get(&id).copied(),
            BlockParent::Root,
            LISTED,
            false
        ),
        "a root block is always listed by the root list itself"
    );
    assert!(can_move_out_of(
        &client,
        &refusing,
        |id| block_types.get(&id).copied(),
        BlockParent::Detached,
        LISTED,
        true
    ));

    let deleting = catalog(ChildOperations {
        add: false,
        delete: true,
        replace: false,
        reorder: false,
    });
    assert!(can_move_out_of(
        &client,
        &deleting,
        |id| block_types.get(&id).copied(),
        BlockParent::Block(CONTAINER),
        LISTED,
        false
    ));
}
