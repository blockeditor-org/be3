use super::*;
use be_sdk_beui::Vec2;

use super::a_dropped_card_takes_its_childs_size_in_the_step_that_added_it::{dropped, report};

#[test]
fn a_childs_first_size_after_another_write_is_a_step_of_its_own() {
    let mut editor = dropped();
    let [placeholder] = entities(&editor)
        .try_into()
        .expect("the drop added one card");

    // The person draws a shape before the child's editor has told its size.
    let canvas = editor.rect_of("infinite-canvas.canvas");
    editor.click("infinite-canvas.dock.tool.Rectangle");
    let from = canvas.min + Vec2::new(40.0, 100.0);
    editor.drag(from, from + Vec2::new(60.0, 40.0));
    assert_eq!(entities(&editor).len(), 2, "the shape is drawn");

    report(&mut editor, Some(Vec2::new(440.0, 480.0)));
    let card = entities(&editor)
        .into_iter()
        .find(|entity| entity.id == placeholder.id)
        .expect("the card is still there");
    assert_ne!(card, placeholder, "the card still takes its child's size");

    undo(&mut editor);
    let held = entities(&editor);
    assert_eq!(held.len(), 2, "the first undo keeps the card and the shape");
    assert!(
        held.contains(&placeholder),
        "and takes back only the size, a step of its own: {held:?}"
    );
}
