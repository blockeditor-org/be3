use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn expanding_a_folder_shows_its_children_without_more_input() {
    let mut fixture = editor();
    let block_type = Uuid::new_v4();
    let folder = Uuid::new_v4();
    let child = Uuid::new_v4();
    let mut listed = BlockInfo::new(folder, block_type, BlockParent::Root);
    listed.references = vec![child];
    fixture.host.set_blocks(BlockQuery::Roots, vec![listed]);
    fixture.settle();

    fixture.test.click(&format!("file-tree.{folder}.chevron"));
    fixture.test.run();
    assert!(
        fixture
            .host
            .watched_blocks()
            .contains(&BlockQuery::References(folder)),
        "the click that expands a folder must ask for its children"
    );

    fixture.host.set_blocks(
        BlockQuery::References(folder),
        vec![BlockInfo::new(
            child,
            block_type,
            BlockParent::Block(folder),
        )],
    );
    fixture.test.run();
    assert!(
        fixture.test.shown(&format!("file-tree.{child}.row")),
        "the children must be drawn in the frame their answer arrives in"
    );
}
