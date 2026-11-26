use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A container whose manifest takes a replaced child can still refuse one at its type: a text
/// that no longer names the block unlinked. It is asked before the copy is made, so no copy is
/// left behind, and the person is told the text still links to the original.
#[test]
fn an_unlink_the_containers_type_refuses_makes_no_copy() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (source, notes) = (Uuid::new_v4(), Uuid::new_v4());
    let text = TextContent::CONTENT_TYPE;
    make(
        source,
        text,
        BlockParent::Root,
        "Source",
        TextBlock::of("source").encode(),
    );
    make(
        notes,
        text,
        BlockParent::Root,
        "Unlinked notes",
        TextBlock::of("no link").encode(),
    );
    synced();
    let _apps = known(&[text]);
    let mut core = core();
    core.block_types.insert(source, text);
    core.block_types.insert(notes, text);

    core.queue_copy(source, notes);
    core.process_pending_copies();
    let copy = match &core.pending_copies[0].stage {
        CopyStage::Replacing { copy_id, .. } => *copy_id,
        _ => panic!("the copy is made and sent to the text before its type answers"),
    };
    settle(&mut core);

    assert!(refused("Unlinked notes can't take its own copy"));
    assert!(
        be_host_stack::node(copy).is_none(),
        "no copy is made, so none is left in Recently Deleted"
    );
}
