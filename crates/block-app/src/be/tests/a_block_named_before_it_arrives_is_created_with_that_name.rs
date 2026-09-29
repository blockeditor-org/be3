use super::*;

#[test]
fn a_block_named_before_it_arrives_is_created_with_that_name() {
    let harness = Harness::start();
    harness.connect();
    let block = Uuid::new_v4();

    name_when_created(block, "Plan".to_owned());
    create(
        block,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );

    let metadata = node(block).expect("the block is in the graph").metadata;
    assert_eq!(metadata.name.as_deref(), Some("Plan"));
    assert!(metadata.named_by_hand, "a name chosen for it is its own");

    let later = Uuid::new_v4();
    create(
        later,
        CounterContent::CONTENT_TYPE,
        be_graph::BlockParent::Root,
        be_block::BlockMetadata::default(),
        None,
    );
    name_when_created(later, "Ideas".to_owned());
    let metadata = node(later).expect("the block is in the graph").metadata;
    assert_eq!(
        metadata.name.as_deref(),
        Some("Ideas"),
        "a block already here is named straight away"
    );
}
