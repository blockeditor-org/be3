use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A container whose manifest takes a replaced child can still refuse one at its type: a text
/// that no longer names the block unlinked. The copy made for it is placed nowhere, and the
/// person is told the text still links to the original.
#[test]
fn an_unlink_the_containers_type_refuses_places_the_copy_nowhere() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (source, notes) = (Uuid::new_v4(), Uuid::new_v4());
    let text = TextContent::CONTENT_TYPE;
    make(source, text, BlockParent::Root, "Source", TextBlock::of("source").encode());
    make(notes, text, BlockParent::Root, "Notes", TextBlock::of("no link").encode());
    synced();
    known(&[text]);
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

    assert!(refused("still links to the original"));
    assert_ne!(
        be_host_stack::node(copy).and_then(|node| node.parent),
        Some(BlockParent::Block(notes)),
        "the copy is not placed in the text"
    );
}
