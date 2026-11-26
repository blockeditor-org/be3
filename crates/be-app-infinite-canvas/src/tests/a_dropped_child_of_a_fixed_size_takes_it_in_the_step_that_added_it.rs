use super::*;
use be_sdk_beui::{ResizeMode, Vec2};

use super::a_dropped_card_takes_its_childs_size_in_the_step_that_added_it::{
    dropped, redo, report_as, shows,
};

#[test]
fn a_dropped_child_of_a_fixed_size_takes_it_in_the_step_that_added_it() {
    // An image, say: the person cannot resize its card, and its child tells its size.
    let mut editor = dropped();
    report_as(&mut editor, Some(Vec2::new(300.0, 200.0)), ResizeMode::None);
    let [card] = entities(&editor)
        .try_into()
        .expect("the drop added one card");
    let size = shows(&card);
    assert!(
        (size.x - 300.0).abs() < 0.5 && (size.y - 200.0).abs() < 0.5,
        "the card takes its child's size: {size:?}"
    );

    undo(&mut editor);
    assert_eq!(
        entities(&editor),
        [],
        "one undo takes the card away, size and all"
    );
    redo(&mut editor);
    assert_eq!(entities(&editor), [card], "one redo brings it back sized");
}
