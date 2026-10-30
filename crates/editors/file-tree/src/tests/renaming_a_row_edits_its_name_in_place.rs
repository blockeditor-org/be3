use block_editor_beui::beui::Key;
use block_editor_beui::beui::icons::ICON_FILE_TEXT;
use block_editor_beui::{
    BlockInfo, BlockParent, BlockTypeDescriptor, Catalog, ChildContent, ChildOperations,
};

use super::*;

#[test]
fn renaming_a_row_edits_its_name_in_place() {
    let mut fixture = editor();
    let notes = Uuid::from_u128(5);
    let kind = Uuid::from_u128(1);
    fixture.test.catalog(Catalog {
        types: vec![BlockTypeDescriptor {
            block_type: kind.into_bytes(),
            display_name: "Note".to_owned(),
            color: None,
            icon_codepoint: ICON_FILE_TEXT.as_str().to_owned(),
            children: ChildOperations::default(),
            press: block_editor_beui::Press::Open,
            plain_file: false,
        }],
        ..Catalog::default()
    });
    let mut listed = BlockInfo::new(notes, kind, BlockParent::Block(MY_FILES));
    listed.set_name("Notes");
    let files = my_files(&mut fixture, vec![listed]);
    fixture.settle();
    let field = format!("file-tree.{notes}.rename");

    fixture.choose(notes, "Rename");
    assert!(fixture.test.shown(&field), "Rename edits the row in place");
    let icon = fixture.test.rect_of(&format!("file-tree.{notes}.row"));
    let edited = fixture.test.rect_of(&field);
    assert!(
        edited.min.x > icon.min.x + 8.0,
        "the field starts after the row's icon, which stays while the name is edited: {edited:?} in {icon:?}"
    );
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
