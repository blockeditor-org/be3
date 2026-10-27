use block_editor_beui::{BlockCommand, ChildOperations};

use super::*;

#[test]
fn a_row_whose_container_keeps_no_order_only_moves_into_folders() {
    let mut fixture = editor();
    containers(
        &mut fixture,
        ChildOperations {
            add: true,
            delete: true,
            replace: true,
            reorder: false,
        },
    );
    let listed = folder_of(&mut fixture, 4, "Folder", Vec::new());
    let folder = listed.id;
    let notes = named_block(5, PLAIN, MY_FILES, "Notes");
    let todo = named_block(7, PLAIN, MY_FILES, "Todo");
    my_files(&mut fixture, vec![listed, notes.clone(), todo.clone()]);
    fixture.settle();
    let resting = fixture.row(todo.id);
    fixture.test.take_block_commands();

    fixture.lift(todo.id);
    let below_notes = across(fixture.row(notes.id), 0.8);
    assert_eq!(
        fixture.carry(todo.id, PLAIN, below_notes),
        Some(false),
        "a workspace that keeps no order takes no row between its children"
    );
    fixture.settle();
    assert_eq!(fixture.row(todo.id), resting, "and no gap opens");

    let top_of_folder = across(fixture.row(folder), 0.1);
    assert_eq!(
        fixture.carry(todo.id, PLAIN, top_of_folder),
        Some(true),
        "the whole of a folder's row takes it inside instead"
    );
    fixture.put_down(todo.id, PLAIN, below_notes);
    assert!(
        fixture
            .test
            .take_block_commands()
            .iter()
            .all(|(_, command)| !matches!(
                command,
                BlockCommand::Reorder { .. } | BlockCommand::Move { .. }
            )),
        "dropped between two rows it asks for nothing"
    );
}
