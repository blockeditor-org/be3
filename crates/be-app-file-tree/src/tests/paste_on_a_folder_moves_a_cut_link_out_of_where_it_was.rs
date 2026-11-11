use be_sdk_beui::{BlockCommand, BlockParent};

use super::*;

#[test]
fn paste_on_a_folder_moves_a_cut_link_out_of_where_it_was() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let listed = folder_of(&mut fixture, 4, "Folder", Vec::new());
    let elsewhere = Uuid::from_u128(9);
    let link = named_block(5, PLAIN, elsewhere, "Linked notes");
    let folder = listed.id;
    my_files(&mut fixture, vec![listed, link.clone()]);
    fixture.settle();

    fixture.choose(link.id, "Cut");
    fixture.test.take_requests();
    fixture.test.take_block_commands();
    paste_into_folder(&mut fixture, folder, link.id, false);
    assert_eq!(
        fixture.test.take_block_commands(),
        vec![(
            link.id,
            BlockCommand::Move {
                block_type: PLAIN.into_bytes(),
                source: BlockParent::Block(MY_FILES).encode(),
                destination: folder.into_bytes(),
                is_reference: true,
            }
        )],
        "the link leaves where it was cut from for the folder"
    );

    paste_into_folder(&mut fixture, folder, link.id, false);
    assert_eq!(
        fixture.test.take_block_commands(),
        vec![(
            link.id,
            BlockCommand::Place {
                block_type: PLAIN.into_bytes(),
                parent: folder.into_bytes(),
                linked: true,
            }
        )],
        "a second paste links it again"
    );
}
