use super::*;

#[test]
fn a_block_made_while_the_server_is_silent_shows_and_takes_edits_at_once() {
    let harness = Harness::relayed().on_manual_clock();
    harness.connect();
    let (folder, counter) = (Uuid::new_v4(), Uuid::new_v4());
    harness.relay().stall();

    create(
        folder,
        be_type_workspace::FolderContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Root,
        be_block_content::BlockMetadata::named("Trip"),
        None,
        false,
    );
    create(
        counter,
        CounterContent::CONTENT_TYPE,
        be_block_graph::BlockParent::Block(folder),
        be_block_content::BlockMetadata::default(),
        None,
        false,
    );
    set_parent(counter, be_block_graph::BlockParent::Root);
    open(counter, CounterContent::CONTENT_TYPE);
    add(counter, 3);
    wait_for_count(counter, 3);
    assert_eq!(
        node(counter).and_then(|node| node.parent),
        Some(be_block_graph::BlockParent::Root),
        "the move shows at once"
    );
    assert!(
        status().connected,
        "making, moving and editing a block waited for the silent server"
    );
    assert!(status().waiting > 0, "what was made waits to be sent");
}
