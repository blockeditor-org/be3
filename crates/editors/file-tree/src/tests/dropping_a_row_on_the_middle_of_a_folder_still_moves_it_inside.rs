use block_editor_beui::{BlockCommand, BlockParent};

use super::*;

#[test]
fn dropping_a_row_on_the_middle_of_a_folder_still_moves_it_inside() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let listed = folder_of(&mut fixture, 4, "Folder", Vec::new());
    let notes = named_block(5, PLAIN, MY_FILES, "Notes");
    let folder = listed.id;
    my_files(&mut fixture, vec![listed, notes.clone()]);
    fixture.settle();
    let resting = fixture.row(notes.id);
    fixture.test.take_block_commands();

    fixture.lift(notes.id);
    let middle = fixture.row(folder).center();
    assert_eq!(fixture.carry(notes.id, PLAIN, middle), Some(true));
    fixture.settle();
    assert_eq!(
        fixture.row(notes.id),
        resting,
        "a drop into a folder opens no gap among the rows"
    );
    fixture.put_down(notes.id, PLAIN, middle);
    assert_eq!(
        fixture.test.take_block_commands(),
        vec![(
            notes.id,
            BlockCommand::Move {
                block_type: PLAIN.into_bytes(),
                source: BlockParent::Block(MY_FILES).encode(),
                destination: folder.into_bytes(),
                is_reference: false,
            }
        )],
        "the middle of a folder takes the row inside it, without placing it"
    );
}
