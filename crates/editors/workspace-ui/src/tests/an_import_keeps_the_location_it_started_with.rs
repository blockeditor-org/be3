use block_editor_beui::be_block::BlockContent;
use block_editor_beui::{BlockCommand, BlockInfo, BlockLocation, ImportedBlock, PickedBlock};
use block_ui_test::HostCall;
use workspace_ui_types::FolderContent;

use super::*;

#[test]
fn an_import_keeps_the_location_it_started_with() {
    let (fixture, _) = editor();
    let folder = Uuid::new_v4();
    let mut info = BlockInfo::new(folder, FolderContent::CONTENT_TYPE, BlockParent::Root);
    info.set_name("Trips".to_owned());
    fixture.test.store().add_block(info);
    let mut placing = filter();
    placing.place = Some(BlockLocation::Root);
    let mut fixture = picking(
        &fixture,
        placing,
        BlockLocation::Block(Uuid::new_v4().into_bytes()),
    );
    fixture.settle();
    fixture.test.take_requests();

    fixture.test.click("picker.import");
    let (request, place) = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ImportBlocks { reply, parent } => Some((reply, parent)),
            _ => None,
        })
        .expect("choosing a file asks the host to import one");
    assert_eq!(
        place,
        BlockLocation::Root,
        "the import goes to the location chosen"
    );

    fixture.test.click("picker.location");
    fixture.test.click(format!("picker.place.{folder}"));

    let imported = Uuid::new_v4();
    fixture.test.answer(
        request,
        Ok(Some(ImportedBlock {
            block_id: imported.into_bytes(),
            block_type: SHOWN_TYPE.into_bytes(),
        })),
    );
    fixture.settle();

    assert_eq!(
        fixture.test.take_picked(),
        vec![Ok(Some(PickedBlock {
            block_id: imported.into_bytes(),
            block_type: SHOWN_TYPE.into_bytes(),
            linked: false,
            placed: true,
        }))]
    );
    assert!(
        !fixture
            .test
            .take_block_commands()
            .iter()
            .any(|(_, command)| matches!(command, BlockCommand::Place { .. })),
        "a location chosen while importing does not move the copy"
    );
}
