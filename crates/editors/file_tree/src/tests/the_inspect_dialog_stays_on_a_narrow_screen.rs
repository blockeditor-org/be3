use block_editor_beui::beui::Vec2;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

const WIDTH: f32 = 280.0;

#[test]
fn the_inspect_dialog_stays_on_a_narrow_screen() {
    let mut fixture = editor_sized(Some(Vec2::new(WIDTH, 700.0)));
    let block = Uuid::from_u128(2);
    let mut listed = BlockInfo::new(block, Uuid::from_u128(1), BlockParent::Root);
    listed.name = Some("Notes".to_owned());
    fixture.host.set_blocks(BlockQuery::Roots, vec![listed]);
    fixture.settle();

    fixture.choose(block, "Inspect");

    for field in ["file-tree.inspect.id", "file-tree.inspect.close"] {
        let rect = fixture.test.rect_of(field);
        assert!(
            rect.left() >= 0.0 && rect.right() <= WIDTH,
            "{field} is on screen, at {rect:?}"
        );
    }
    fixture
        .test
        .snapshot("the_inspect_dialog_stays_on_a_narrow_screen");
}
