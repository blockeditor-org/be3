use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockInfo, BlockLocation};
use block_ui_test::HostCall;
use workspace_ui_types::FolderContent;

use super::*;

#[test]
fn an_import_into_a_folder_is_made_inside_it() {
    let (fixture, _) = editor();
    let folder = Uuid::new_v4();
    let mut info = BlockInfo::new(folder, FolderContent::CONTENT_TYPE, BlockParent::Root);
    info.set_name("Trips".to_owned());
    fixture.test.store().add_block(info);
    let mut placing = filter();
    placing.place = Some(BlockLocation::Block(folder.into_bytes()));
    let mut fixture = picking(&fixture, placing, BlockLocation::Detached);
    fixture.settle();
    fixture.test.take_requests();

    fixture.test.click("picker.import");
    fixture.settle();
    let place = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ImportBlocks { parent, .. } => Some(parent),
            _ => None,
        })
        .expect("choosing a file asks the host to import one");
    assert_eq!(
        place,
        BlockLocation::Block(folder.into_bytes()),
        "the copy is made in the folder, so it never has to move there"
    );
}
