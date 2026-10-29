use block_editor_beui::DialogAnswer;
use block_editor_beui::beui::{Key, Modifiers};

use super::*;

#[test]
fn the_rename_form_saves_a_typed_name_and_closes_on_enter() {
    let (workspace, block) = editor();
    let mut fixture = renaming(&workspace, block);
    assert!(fixture.says("Rename"));

    fixture.test.click("rename.name");
    fixture.settle();
    fixture.test.type_text("Notes");
    fixture.settle();
    assert_eq!(
        fixture.test.take_renames().last(),
        Some(&(block, Some("Notes".to_owned()))),
        "the typed name is saved to the block as it is typed"
    );

    fixture.test.key(Key::Enter, Modifiers::NONE);
    fixture.settle();
    assert_eq!(
        fixture.test.take_dialog_closes(),
        [DialogAnswer::Accepted],
        "Enter tells the editor that placed the form it is done"
    );
}
