use block_editor_beui::{BlockCommand, BlockQuery};

use super::*;

#[test]
fn a_row_carried_among_its_siblings_parts_them_and_is_reordered_on_drop() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let notes = named_block(5, PLAIN, MY_FILES, "Notes");
    let plans = named_block(6, PLAIN, MY_FILES, "Plans");
    let todo = named_block(7, PLAIN, MY_FILES, "Todo");
    my_files(
        &mut fixture,
        vec![notes.clone(), plans.clone(), todo.clone()],
    );
    fixture.settle();
    let resting = fixture.row(plans.id);
    let last = fixture.row(todo.id);

    fixture.lift(todo.id);
    let below_notes = across(fixture.row(notes.id), 0.8);
    assert_eq!(
        fixture.carry(todo.id, PLAIN, below_notes),
        Some(true),
        "the lower half of a sibling takes the row after it"
    );
    fixture.test.record();
    for _ in 0..6 {
        fixture.test.advance(std::time::Duration::from_millis(16));
        fixture.test.record();
    }
    fixture.test.settle();
    let parted = fixture.row(plans.id);
    assert!(
        parted.top() >= resting.bottom(),
        "Plans glides down a whole row to open a gap after Notes: {resting:?} -> {parted:?}"
    );
    fixture
        .test
        .snapshot("a_row_carried_among_its_siblings_parts_them_and_is_reordered_on_drop");

    fixture.test.take_block_commands();
    fixture.put_down(todo.id, PLAIN, below_notes);
    let asked = fixture.test.take_block_commands();
    assert_eq!(
        asked,
        vec![(
            todo.id,
            BlockCommand::Reorder {
                parent: MY_FILES.into_bytes(),
                after: Some(notes.id.into_bytes()),
            }
        )],
        "the drop asks the workspace to put Todo after Notes, and nothing else"
    );
    assert!(
        fixture.row(plans.id).top() >= resting.bottom(),
        "the gap stays open until the workspace lists its children again"
    );

    fixture.host.set_blocks(
        BlockQuery::References(MY_FILES),
        vec![notes.clone(), todo.clone(), plans.clone()],
    );
    fixture.test.settle();
    assert_eq!(
        (fixture.row(todo.id), fixture.row(plans.id)),
        (resting, last),
        "once listed in the new order, the gap closes with Todo in it"
    );
}
