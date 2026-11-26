use be_sdk_beui::{BlockInfo, BlockParent};

use super::*;

#[test]
fn a_rename_ends_as_left_once_the_keyboard_goes_to_another_block() {
    let mut fixture = editor();
    let notes = Uuid::from_u128(5);
    let mut listed = BlockInfo::new(notes, PLAIN, BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    my_files(&mut fixture, vec![listed]);
    fixture.settle();
    let field = format!("file-tree.{notes}.rename");

    fixture.choose(notes, "Rename");
    assert!(fixture.test.shown(&field), "Rename edits the row in place");
    fixture.test.type_text("Plans");
    fixture.settle();
    fixture.test.blur();
    fixture.settle();
    assert!(
        !fixture.test.shown(&field),
        "the keyboard going to another block ends the edit"
    );
    assert_eq!(
        fixture.test.take_renames(),
        vec![(notes, Some("Plans".to_owned()))],
        "it ends as leaving the field does, keeping what was typed"
    );

    fixture.test.focus();
    fixture.settle();
    assert!(
        !fixture.test.shown(&field),
        "the keyboard coming back does not reopen the field"
    );
}
