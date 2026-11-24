use be_sdk_beui::beui::{Key, Modifiers};
use be_sdk_beui::{BlockInfo, BlockParent, BlockQuery};

use super::*;

/// Listed as rows, an entry's row menu renames it in place, as the files pane's does: Enter names
/// the block what was typed.
#[test]
fn an_entry_s_row_menu_renames_it_in_place() {
    let (Fixture { mut editor, folder }, children) = editor(1);
    let notes = children[0];
    let mut listed = BlockInfo::new(notes, Uuid::from_u128(1), BlockParent::Block(folder));
    listed.set_name("Notes");
    editor
        .host()
        .set_blocks(BlockQuery::References(folder), vec![listed]);
    editor.run();
    editor.click("folder.view");
    editor.run();
    click_item(&mut editor, "List");

    let entry = format!("folder.entry.{notes}");
    editor.click(format!("{entry}.more"));
    editor.run();
    click_item(&mut editor, "Rename");
    let field = format!("{entry}.rename");
    assert!(editor.shown(&field), "Rename edits the row in place");
    editor.snapshot("an_entry_s_row_menu_renames_it_in_place");
    editor.type_text("Plans");
    editor.run();
    editor.key(Key::Enter, Modifiers::NONE);
    editor.run();
    assert!(!editor.shown(&field), "Enter ends the edit");
    assert_eq!(
        editor.take_renames(),
        vec![(notes, Some("Plans".to_owned()))],
        "Enter names the block what was typed over its old name"
    );
}
