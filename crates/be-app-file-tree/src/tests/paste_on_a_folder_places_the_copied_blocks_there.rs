use be_sdk_beui::beui::clipboard::{Blocks, ClipBlock, ClipFormat as _};
use be_sdk_beui::{BlockCommand, BlockParent, PastedBlock};
use be_sdk_test::HostCall;

use super::*;

fn paste(fixture: &mut Fixture, folder: Uuid, block: Uuid, moved: bool) {
    fixture.choose(folder, "Paste");
    let copied = Blocks {
        blocks: vec![ClipBlock {
            id: block,
            organization: None,
            block_type: PLAIN,
            title: "Notes".to_owned(),
        }],
    };
    fixture.test.step(vec![Input::Paste(copied.encode())]);
    fixture.settle();
    let asked = fixture
        .test
        .take_requests()
        .into_iter()
        .filter_map(|call| match call {
            HostCall::PasteBlocks {
                reply,
                blocks,
                parent,
                ..
            } => Some((reply, blocks, parent)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [(reply, blocks, parent)]: [_; 1] = asked
        .try_into()
        .unwrap_or_else(|asked: Vec<_>| panic!("the paste asks the host once, asked {asked:?}"));
    assert_eq!(
        (blocks, parent),
        (vec![block.into_bytes()], BlockParent::Block(folder).encode()),
        "the clip goes into the folder the menu was opened on"
    );
    fixture.test.answer(
        reply,
        Ok(vec![PastedBlock {
            block_id: block.into_bytes(),
            block_type: PLAIN.into_bytes(),
            moved,
        }]),
    );
    fixture.settle();
}

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
    fixture.test.take_block_commands();

    paste(&mut fixture, folder, notes.id, false);
    assert_eq!(
        fixture.test.take_block_commands(),
        vec![(
            notes.id,
            BlockCommand::Place {
                block_type: PLAIN.into_bytes(),
                parent: folder.into_bytes(),
                linked: true,
            }
        )],
        "a copied block is linked into the folder"
    );

    paste(&mut fixture, folder, notes.id, true);
    assert_eq!(
        fixture.test.take_block_commands(),
        Vec::new(),
        "a cut block the host moved there is not placed again"
    );
}
