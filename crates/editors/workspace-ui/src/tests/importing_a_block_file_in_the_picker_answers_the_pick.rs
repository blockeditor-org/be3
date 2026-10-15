use block_editor_beui::{BlockImport, BlockLocation, BlockPick, HostReply, HostRequest};

use super::*;

#[test]
fn importing_a_block_file_in_the_picker_answers_the_pick() {
    let (mut fixture, _) = editor();
    let parent = Uuid::new_v4();
    fixture
        .test
        .request_pick(4, filter(), BlockLocation::Block(parent.into_bytes()));
    fixture.settle();
    fixture.test.take_requests();

    fixture.click_text("Import");
    fixture
        .test
        .snapshot("importing_a_block_file_in_the_picker_answers_the_pick");
    fixture.test.click("picker.import");
    fixture.settle();

    let imports: Vec<_> = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|(id, request)| match request {
            HostRequest::ImportBlocks(place) => Some((id, place)),
            _ => None,
        })
        .collect();
    let [(request, place)] = imports.as_slice() else {
        panic!("choosing a file asks the host to import one, asked {imports:?}");
    };
    assert_eq!(
        *place,
        BlockLocation::Block(parent.into_bytes()),
        "the import lands where a created block would"
    );
    assert!(fixture.says("Importing..."), "the picker waits for the file");

    fixture
        .test
        .reply(*request, HostReply::BlocksImported(BlockImport::Cancelled));
    fixture.settle();
    assert!(
        fixture.says("Choose a block file"),
        "cancelling the file picker leaves the picker open to try again"
    );
    assert!(fixture.test.take_pick_answers().is_empty());

    fixture.test.click("picker.import");
    fixture.settle();
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|(id, request)| matches!(request, HostRequest::ImportBlocks(_)).then_some(id))
        .expect("choosing again asks again");
    let imported = Uuid::new_v4();
    fixture.test.reply(
        request,
        HostReply::BlocksImported(BlockImport::Imported {
            block_id: imported.into_bytes(),
            block_type: SHOWN_TYPE.into_bytes(),
        }),
    );
    fixture.settle();

    assert_eq!(
        fixture.test.take_pick_answers(),
        vec![(
            4,
            BlockPick::Chosen {
                block_id: imported.into_bytes(),
                block_type: SHOWN_TYPE.into_bytes(),
                linked: false,
                placed: false,
            }
        )]
    );
    assert!(
        !fixture.says("Add block"),
        "an answered pick closes the picker"
    );
}
