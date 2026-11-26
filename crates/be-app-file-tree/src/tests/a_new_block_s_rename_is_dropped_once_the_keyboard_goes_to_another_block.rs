use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::{BlockInfo, BlockParent, BlockQuery, PickedBlock};
use be_type_workspace::FolderContent;

use super::*;

#[test]
fn a_new_block_s_rename_is_dropped_once_the_keyboard_goes_to_another_block() {
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
        named: false,
        placed: false,
    })));
    fixture.settle();
    fixture.test.blur();
    fixture.settle();

    let mut untitled = BlockInfo::new(made, Uuid::from_u128(7), BlockParent::Block(folder));
    untitled.set_intrinsic_name("Untitled".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![untitled]);
    fixture.settle();
    let field = format!("file-tree.{made}.rename");
    assert!(fixture.test.shown(&format!("file-tree.{made}.row")));
    assert!(
        !fixture.test.shown(&field),
        "the keyboard went to another block before the new row showed, so no name field opens"
    );

    fixture.test.focus();
    fixture.settle();
    assert!(
        !fixture.test.shown(&field),
        "the keyboard coming back does not bring the dropped rename with it"
    );
    assert_eq!(fixture.test.take_renames(), Vec::new());
}
