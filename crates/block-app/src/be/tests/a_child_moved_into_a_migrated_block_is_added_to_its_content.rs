use super::*;

use be_block::{BlockRef, ChildChange, PresentationContent};
use block::Block;
use block_client::blocks::presentation::Presentation;

fn slides_of(shared: &Shared, block: Uuid) -> Option<Vec<Option<BlockRef>>> {
    let held = shared.blocks.get(&block)?;
    let presentation = PresentationContent::decode(&held.bytes).ok()?;
    Some(
        presentation
            .root()
            .slides
            .iter()
            .map(|slide| slide.block)
            .collect(),
    )
}

#[test]
fn a_child_moved_into_a_migrated_block_is_added_to_its_content() {
    let harness = Harness::start();
    harness.connect();
    let (block, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());

    assert_eq!(
        change_child(block, Presentation::TYPE_ID, ChildChange::Add(first)),
        None
    );
    wait_until("opened the block to change", |shared| {
        slides_of(shared, block) == Some(Vec::new())
    });

    assert_eq!(
        change_child(block, Presentation::TYPE_ID, ChildChange::Add(first)),
        Some(true)
    );
    wait_until("added the child", |shared| {
        slides_of(shared, block) == Some(vec![Some(BlockRef::Direct(first))])
    });

    assert_eq!(
        change_child(
            block,
            Presentation::TYPE_ID,
            ChildChange::Replace {
                old: first,
                new: second
            }
        ),
        Some(true)
    );
    wait_until("replaced the child", |shared| {
        slides_of(shared, block) == Some(vec![Some(BlockRef::Direct(second))])
    });
}
