use super::*;
use uuid::Uuid;

use be_block::ChildChange;
use presentation_types::PresentationContent;

fn slides_of(shared: &Shared, block: Uuid) -> Option<Vec<Option<Uuid>>> {
    let held = shared.blocks.get(&block)?;
    let presentation = PresentationContent::decode(held.bytes()).ok()?;
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
fn a_child_moved_into_a_block_is_added_to_its_content() {
    let harness = Harness::start();
    harness.connect();
    let (block, first, second) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());

    wait_until("knew the block's type", |_| {
        is_known(PresentationContent::CONTENT_TYPE)
    });
    assert_eq!(
        change_child(
            block,
            PresentationContent::CONTENT_TYPE,
            ChildChange::Add(first)
        ),
        Some(true)
    );
    wait_until("added the child", |shared| {
        slides_of(shared, block) == Some(vec![Some(first)])
    });

    assert_eq!(
        change_child(
            block,
            PresentationContent::CONTENT_TYPE,
            ChildChange::Replace {
                old: first,
                new: second
            }
        ),
        Some(true)
    );
    wait_until("replaced the child", |shared| {
        slides_of(shared, block) == Some(vec![Some(second)])
    });
}
