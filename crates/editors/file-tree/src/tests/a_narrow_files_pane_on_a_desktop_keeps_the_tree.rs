use block_editor_beui::beui::Vec2;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_narrow_files_pane_on_a_desktop_keeps_the_tree() {
    let mut fixture = editor_sized(Some(Vec2::new(260.0, 700.0)));
    let block = Uuid::from_u128(2);
    let mut listed = BlockInfo::new(block, Uuid::from_u128(1), BlockParent::Root);
    listed.set_intrinsic_name("Notes".to_owned());
    fixture.host.set_blocks(BlockQuery::Roots, vec![listed]);
    fixture.settle();

    assert!(fixture.test.shown("file-tree.app-menu"));
    let compact = fixture.test.rect_of("file-tree.shared.row").height();

    let mut phone = phone();
    phone.host.set_blocks(BlockQuery::Roots, Vec::new());
    phone.settle();
    let comfortable = phone.test.rect_of("file-tree.shared.row").height();
    assert!(
        compact < comfortable,
        "a narrow pane keeps the desktop's rows, {compact} high, rather than a phone's {comfortable}"
    );
}
