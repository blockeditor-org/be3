use be_sdk_beui::BlockCommand;

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
    fixture.test.take_block_commands();

    paste_into_folder(&mut fixture, folder, notes.id, false, None);
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

    paste_into_folder(&mut fixture, folder, notes.id, true, None);
    assert_eq!(
        fixture.test.take_block_commands(),
        Vec::new(),
        "a cut block the host moved there is not placed again"
    );
}
