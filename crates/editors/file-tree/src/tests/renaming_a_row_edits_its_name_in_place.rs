use block_editor_beui::beui::Key;
use block_editor_beui::{BlockInfo, BlockParent, ChildContent};

use super::*;

#[test]
fn renaming_a_row_edits_its_name_in_place() {
    let mut fixture = editor();
    let notes = Uuid::from_u128(5);
    let mut listed = BlockInfo::new(notes, Uuid::from_u128(1), BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    let files = my_files(&mut fixture, vec![listed]);
    fixture.settle();
    let field = format!("file-tree.{notes}.rename");

    fixture.choose(notes, "Rename");
    assert!(fixture.test.shown(&field), "Rename edits the row in place");
    fixture
        .test
        .snapshot("renaming_a_row_edits_its_name_in_place");
    fixture.test.type_text("Plans");
    fixture.settle();
    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert!(!fixture.test.shown(&field), "Enter ends the edit");
    assert_eq!(
        fixture.test.take_renames(),
        vec![(notes, Some("Plans".to_owned()))],
        "Enter names the block what was typed over its old name"
    );

    fixture.test.key(Key::F2, Modifiers::NONE);
    fixture.settle();
    assert!(
        fixture.test.shown(&field),
        "F2 on the row the keyboard is on edits it"
    );
    fixture.test.type_text("Ideas");
    fixture.settle();
    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    assert!(!fixture.test.shown(&field), "Escape ends the edit");
    assert!(
        fixture.test.take_renames().is_empty(),
        "Escape leaves the name alone"
    );

    fixture.test.key(Key::F2, Modifiers::NONE);
    fixture.settle();
    fixture.test.type_text("Ideas");
    fixture.settle();
    fixture.test.click(format!("file-tree.{files}.row"));
    fixture.settle();
    assert!(!fixture.test.shown(&field), "clicking away ends the edit");
    assert_eq!(
        fixture.test.take_renames(),
        vec![(notes, Some("Ideas".to_owned()))],
        "clicking away keeps what was typed"
    );
    assert!(
        fixture
            .test
            .placed(|content| matches!(content, ChildContent::Rename(_)))
            .is_none(),
        "no rename form is placed"
    );
}
