use block_editor_beui::beui::Vec2;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_narrow_files_pane_on_a_desktop_keeps_the_tree() {
    let mut fixture = editor_sized(Some(Vec2::new(260.0, 700.0)));
    let block = Uuid::from_u128(2);
    let mut listed = BlockInfo::new(block, Uuid::from_u128(1), BlockParent::Root);
    listed.name = Some("Notes".to_owned());
    fixture.host.set_blocks(BlockQuery::Roots, vec![listed]);
    fixture.settle();

    assert!(fixture.test.shown("file-tree.add-root"));
    assert!(
        !fixture.test.shown("file-tree.search"),
        "the phone page is only for a phone"
    );
}
