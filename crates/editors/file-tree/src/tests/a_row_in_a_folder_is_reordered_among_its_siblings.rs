use block_editor_beui::BlockCommand;

use super::*;

#[test]
fn a_row_in_a_folder_is_reordered_among_its_siblings() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let folder = Uuid::from_u128(4);
    let first = named_block(5, PLAIN, folder, "First");
    let second = named_block(6, PLAIN, folder, "Second");
    let third = named_block(7, PLAIN, folder, "Third");
    let listed = folder_of(
        &mut fixture,
        4,
        "Folder",
        vec![first.clone(), second.clone(), third.clone()],
    );
    my_files(&mut fixture, vec![listed.clone()]);
    fixture.settle();
    expand(
        &mut fixture,
        &listed,
        vec![first.clone(), second.clone(), third.clone()],
    );
    fixture.test.take_block_commands();

    fixture.lift(third.id);
    let above_first = across(fixture.row(first.id), 0.1);
    assert_eq!(fixture.carry(third.id, PLAIN, above_first), Some(true));
    fixture.put_down(third.id, PLAIN, above_first);
    assert_eq!(
        fixture.test.take_block_commands(),
        vec![(
            third.id,
            BlockCommand::Reorder {
                parent: folder.into_bytes(),
                after: None,
            }
        )],
        "the top of the first child puts the row first in its folder"
    );
}
