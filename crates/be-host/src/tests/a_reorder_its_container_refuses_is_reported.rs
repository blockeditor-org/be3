use be_block_content::BlockContent;
use be_type_text::{TextBlock, TextContent};

use super::*;

/// A reorder its container refuses (a text keeps no order of the blocks it names) is reported,
/// not dropped as if it were made. No type refuses one at its type yet, so this refusal is its
/// manifest's; both come back the same way (`be_host_stack::ChildChanged`).
#[test]
fn a_reorder_its_container_refuses_is_reported() {
    let harness = be_host_testing::Harness::start();
    harness.connect();
    let (notes, child) = (Uuid::new_v4(), Uuid::new_v4());
    let text = TextContent::CONTENT_TYPE;
    make(notes, text, BlockParent::Root, "Notes", TextBlock::of("notes").encode());
    make(child, text, BlockParent::Root, "Child", TextBlock::of("child").encode());
    synced();
    let _apps = known(&[text]);
    let mut core = core();
    core.block_types.insert(notes, text);

    core.queue_reorder(child, notes, None);
    settle(&mut core);

    assert!(refused("Notes can't move Child within it"));
}
