use super::*;

#[test]
fn deleting_needs_a_container_that_can_delete_children() {
    let client = client();
    let block_types = block_types();

    let refusing = catalog(ChildEdits::default());
    assert!(!can_move_out_of(
        &client,
        &refusing,
        &block_types,
        BlockSource::Block(CONTAINER),
        LISTED,
        false
    ));
    assert!(
        can_move_out_of(
            &client,
            &refusing,
            &block_types,
            BlockSource::Root,
            LISTED,
            false
        ),
        "a root block is always listed by the root list itself"
    );
    assert!(can_move_out_of(
        &client,
        &refusing,
        &block_types,
        BlockSource::Orphaned,
        LISTED,
        true
    ));

    let deleting = catalog(ChildEdits {
        add: false,
        delete: true,
        replace: false,
    });
    assert!(can_move_out_of(
        &client,
        &deleting,
        &block_types,
        BlockSource::Block(CONTAINER),
        LISTED,
        false
    ));
}
