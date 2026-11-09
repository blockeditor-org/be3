use be_sdk_beui::BlockParent;
use be_sdk_beui::beui::clipboard::{Blocks, ClipBlock, ClipFormat as _};
use be_sdk_test::HostCall;

use super::*;

#[test]
fn paste_on_a_folder_places_the_copied_blocks_there() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let listed = folder_of(&mut fixture, 4, "Folder", Vec::new());
    let notes = named_block(5, PLAIN, MY_FILES, "Notes");
    let folder = listed.id;
    my_files(&mut fixture, vec![listed, notes.clone()]);
    fixture.settle();
    fixture.test.take_requests();

    fixture.choose(folder, "Paste");
    let copied = Blocks {
        blocks: vec![ClipBlock {
            id: notes.id,
            organization: None,
            block_type: PLAIN,
            title: "Notes".to_owned(),
        }],
    };
    fixture.test.step(vec![Input::Paste(copied.encode())]);
    fixture.settle();

    let pasted = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::PasteBlocks { blocks, parent, .. } => Some((blocks, parent)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        pasted,
        [(
            vec![notes.id.into_bytes()],
            BlockParent::Block(folder).encode()
        )],
        "the clip goes into the folder the menu was opened on"
    );
}
