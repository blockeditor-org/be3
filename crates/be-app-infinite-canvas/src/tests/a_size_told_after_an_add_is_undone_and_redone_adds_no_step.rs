use super::*;
use be_sdk_beui::Vec2;

use super::a_dropped_card_takes_its_childs_size_in_the_step_that_added_it::{dropped, report};

#[test]
fn a_size_told_after_an_add_is_undone_and_redone_adds_no_step() {
    let mut editor = dropped();
    let added = entities(&editor);
    assert_eq!(added.len(), 1, "the drop added one card");

    undo(&mut editor);
    assert_eq!(entities(&editor), [], "undo takes the card away");
    let block = editor.block_id().expect("the canvas has a block");
    editor.host().redo(block);
    editor.run();
    assert_eq!(entities(&editor), added, "redo brings it back");

    report(&mut editor, Some(Vec2::new(440.0, 480.0)));
    assert_eq!(
        entities(&editor),
        added,
        "the size told after the add's step ended leaves the card as it is"
    );
    undo(&mut editor);
    assert_eq!(entities(&editor), [], "and adds no undo step");
}
