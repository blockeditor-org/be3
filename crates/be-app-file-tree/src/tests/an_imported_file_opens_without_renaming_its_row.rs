use be_sdk_beui::be_block_content::BlockContent;
use be_sdk_beui::{BlockInfo, BlockParent, BlockQuery, PickedBlock};
use be_type_workspace::FolderContent;

use super::*;

#[test]
fn an_imported_file_opens_without_renaming_its_row() {
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
    let imported = Uuid::from_u128(11);
    fixture.test.answer_picker(Ok(Some(PickedBlock {
        block_id: imported.into_bytes(),
        block_type: Uuid::from_u128(7).into_bytes(),
        linked: false,
        named: true,
        placed: false,
    })));
    fixture.settle();

    let mut notes = BlockInfo::new(imported, Uuid::from_u128(7), BlockParent::Block(folder));
    notes.set_intrinsic_name("notes.md".to_owned());
    fixture
        .host
        .set_blocks(BlockQuery::References(folder), vec![notes]);
    fixture.settle();
    assert!(fixture.test.shown(&format!("file-tree.{imported}.row")));
    assert!(
        !fixture.test.shown(&format!("file-tree.{imported}.rename")),
        "an imported file keeps the file's name, so its row does not open a name field, which on \
         a phone would raise the keyboard over the files list"
    );
}
