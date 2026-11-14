use super::*;

use be_block_content::{DELETED_FROM, DeletedFrom};
use be_block_graph::BlockParent;

#[test]
fn a_block_deleted_from_unfiled_remembers_it_and_apps_never_see_its_id() {
    let harness = Harness::start();
    harness.connect();
    let unfiled = crate::unfiled().expect("signed in");
    create(
        unfiled,
        be_block::UNFILED_TYPE,
        BlockParent::Root,
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    let note = Uuid::now_v7();
    create(
        note,
        CounterContent::CONTENT_TYPE,
        BlockParent::Block(unfiled),
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    assert_eq!(
        crate::app_graph::parent_of(be_sdk_api::BlockLocation::Unfiled),
        Some(BlockParent::Block(unfiled)),
        "an app's Unfiled is the account's own"
    );
    let read = crate::app_graph::answer(be_sdk_api::BlockQuery::Unfiled);
    assert_eq!(
        read.iter().map(|info| info.block_id).collect::<Vec<_>>(),
        vec![note.into_bytes()],
        "an app lists what is in Unfiled"
    );
    assert_eq!(
        read[0].parent,
        Some(be_sdk_api::BlockLocation::Unfiled),
        "and reads its parent as Unfiled, never by its id"
    );
    assert!(
        crate::app_graph::answer(be_sdk_api::BlockQuery::Roots)
            .iter()
            .all(|info| info.block_id != unfiled.into_bytes()),
        "nor does Unfiled itself show at the top"
    );

    set_parent(note, BlockParent::Detached);
    assert_eq!(
        node(note).and_then(|node| node.metadata.get(&DELETED_FROM)),
        Some(DeletedFrom::Block(unfiled)),
        "deleting it remembers Unfiled, so Restore puts it back"
    );
}
