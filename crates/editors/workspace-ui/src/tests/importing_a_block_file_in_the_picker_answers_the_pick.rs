use block_editor_beui::{BlockLocation, ImportedBlock, PickedBlock};
use block_ui_test::HostCall;

use super::*;

#[test]
fn importing_a_block_file_in_the_picker_answers_the_pick() {
    let (mut fixture, _) = editor();
    let parent = Uuid::new_v4();
    fixture
        .test
        .request_pick(4, filter(), BlockLocation::Block(parent.into_bytes()), None);
    fixture.settle();
    fixture.test.take_requests();

    fixture.test.click("picker.import");

    let imports: Vec<_> = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::ImportBlocks { reply, parent } => Some((reply, parent)),
            _ => None,
        })
        .collect();
    let [(request, place)]: [_; 1] = imports.try_into().unwrap_or_else(|imports: Vec<_>| {
        panic!("choosing a file asks the host to import one, asked {imports:?}")
    });
    assert_eq!(
        place,
        BlockLocation::Block(parent.into_bytes()),
        "the import lands where a created block would"
    );
    assert!(fixture.says("Importing…"), "the picker waits for the file");

    fixture.test.answer(request, Ok(None));
    fixture.settle();
    assert!(
        fixture.says("Import a file…"),
        "cancelling the file picker leaves the menu open to try again"
    );
    assert!(fixture.test.take_pick_answers().is_empty());

    fixture.test.click("picker.import");
    let request = fixture
        .test
        .take_requests()
        .into_iter()
        .find_map(|call| match call {
            HostCall::ImportBlocks { reply, .. } => Some(reply),
            _ => None,
        })
        .expect("choosing again asks again");
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
        fixture.test.take_pick_answers(),
        vec![(
            4,
            Ok(Some(PickedBlock {
                block_id: imported.into_bytes(),
                block_type: SHOWN_TYPE.into_bytes(),
                linked: false,
                placed: false,
            }))
        )]
    );
    assert!(
        !fixture.test.shown("picker.import"),
        "an answered pick closes the menu"
    );
}
