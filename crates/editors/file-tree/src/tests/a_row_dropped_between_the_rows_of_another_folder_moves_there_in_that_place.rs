use block_editor_beui::{BlockCommand, BlockParent};

use super::*;

#[test]
fn a_row_dropped_between_the_rows_of_another_folder_moves_there_in_that_place() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let folder = Uuid::from_u128(4);
    let first = named_block(5, PLAIN, folder, "First");
    let second = named_block(6, PLAIN, folder, "Second");
    let listed = folder_of(
        &mut fixture,
        4,
        "Folder",
        vec![first.clone(), second.clone()],
    );
    let loose = named_block(8, PLAIN, MY_FILES, "Loose");
    my_files(&mut fixture, vec![listed.clone(), loose.clone()]);
    fixture.settle();
    expand(&mut fixture, &listed, vec![first.clone(), second.clone()]);
    fixture.test.take_block_commands();

    fixture.lift(loose.id);
    let above_second = across(fixture.row(second.id), 0.2);
    assert_eq!(fixture.carry(loose.id, PLAIN, above_second), Some(true));
    fixture.put_down(loose.id, PLAIN, above_second);
    assert_eq!(
        fixture.test.take_block_commands(),
        vec![
            (
                loose.id,
                BlockCommand::Move {
                    block_type: PLAIN.into_bytes(),
                    source: BlockParent::Block(MY_FILES).encode(),
                    destination: folder.into_bytes(),
                    is_reference: false,
                }
            ),
            (
                loose.id,
                BlockCommand::Reorder {
                    parent: folder.into_bytes(),
                    after: Some(first.id.into_bytes()),
                }
            ),
        ],
        "the row moves into the folder, then takes the place between First and Second"
    );
}
