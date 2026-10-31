use block_editor_beui::be_block::BlockContent;
use block_editor_beui::beui::Key;
use block_editor_beui::{BlockInfo, BlockParent, BlockQuery, PickedBlock};
use workspace_ui_types::FolderContent;

use super::*;

#[test]
fn enter_right_after_choosing_a_new_block_confirms_its_name() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let folder = Uuid::from_u128(2);
    let mut projects = BlockInfo::new(
        folder,
        FolderContent::CONTENT_TYPE,
        BlockParent::Block(MY_FILES),
    );
    projects.set_intrinsic_name("Projects".to_owned());
    my_files(&mut fixture, vec![projects]);
    fixture.settle();
    fixture.test.click(format!("file-tree.{folder}.row"));
    fixture.settle();
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), Vec::new());
    fixture.settle();

    fixture.test.click("file-tree.new");
    fixture.settle();
    let made = Uuid::from_u128(11);
    fixture.test.answer_picker(Ok(Some(PickedBlock {
        block_id: made.into_bytes(),
        block_type: Uuid::from_u128(7).into_bytes(),
        linked: false,
        placed: false,
    })));
    fixture.settle();
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert!(
        fixture.test.placed_picker().is_none(),
        "Enter before the new block's row shows does not reach New and open the picker again"
    );

    let mut untitled = BlockInfo::new(made, Uuid::from_u128(7), BlockParent::Block(folder));
    untitled.set_intrinsic_name("Untitled".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![untitled]);
    fixture.settle();
    assert!(fixture.test.shown(&format!("file-tree.{made}.row")));
    assert!(
        !fixture.test.shown(&format!("file-tree.{made}.rename")),
        "the Enter confirmed the name it was made with, so no name field opens once the row shows"
    );
    assert_eq!(fixture.test.take_renames(), Vec::new());
}
