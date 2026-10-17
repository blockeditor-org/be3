use block_editor_beui::{AccessLevel, BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_linked_block_it_cannot_see_shows_no_access_rather_than_locked() {
    let mut fixture = editor();
    let text_type = Uuid::from_u128(7);
    let (notes, hidden) = (Uuid::from_u128(3), Uuid::from_u128(4));
    let mut written = BlockInfo::new(notes, text_type, BlockParent::Block(MY_FILES));
    written.set_intrinsic_name("Notes");
    let mut unseen = BlockInfo::new(hidden, text_type, BlockParent::Root);
    unseen.parent = None;
    unseen.author = None;
    unseen.access = AccessLevel::KnowExists;
    let inside = vec![written, unseen];
    let workspace = files_workspace(&fixture.host, &inside);
    fixture.host.set_blocks(BlockQuery::Roots, vec![workspace]);
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(MY_FILES), inside);
    fixture.settle();

    assert!(fixture.says("Notes"));
    assert!(fixture.says("No access"), "a block it may not view says so");
    assert!(
        !fixture.says("Locked"),
        "a block it may not view is not one whose key is on its way"
    );
}
