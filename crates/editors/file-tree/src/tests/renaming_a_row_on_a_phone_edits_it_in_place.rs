use block_editor_beui::beui::Key;
use block_editor_beui::{BlockInfo, BlockParent};

use super::*;

#[test]
fn renaming_a_row_on_a_phone_edits_it_in_place() {
    let mut fixture = phone();
    let notes = Uuid::from_u128(5);
    let mut listed = BlockInfo::new(notes, Uuid::from_u128(1), BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    my_files(&mut fixture, vec![listed]);
    fixture.settle();
    let field = format!("file-tree.{notes}.rename");

    fixture.test.click(format!("file-tree.{notes}.more"));
    fixture.settle();
    fixture.click_item("Rename");
    fixture.test.settle();
    assert!(
        fixture.test.shown(&field),
        "a phone edits the name in its row too"
    );
    let row = fixture.test.rect_of(&format!("file-tree.{notes}.row"));
    let edited = fixture.test.rect_of(&field);
    assert!(
        edited.height() <= row.height() && edited.height() >= row.height() - 4.0,
        "the field is as tall as the row it edits, {} in {}",
        edited.height(),
        row.height()
    );
    fixture
        .test
        .snapshot("renaming_a_row_on_a_phone_edits_it_in_place");
    fixture.test.type_text("Plans");
    fixture.settle();
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.test.take_renames(),
        vec![(notes, Some("Plans".to_owned()))]
    );
}
