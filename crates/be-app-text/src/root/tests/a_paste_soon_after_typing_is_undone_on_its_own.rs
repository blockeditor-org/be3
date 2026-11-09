use super::*;
use be_sdk_beui::beui::{ClipItem, Input};

#[test]
fn a_paste_soon_after_typing_is_undone_on_its_own() {
    let mut editor = editor("one");
    editor.store().keep_history::<TextContent>(None);
    editor.run();
    caret_at(&mut editor, 3);

    editor.type_text(" two");
    editor.run();
    editor.wait(std::time::Duration::from_millis(300));
    editor.step(vec![Input::Paste(ClipItem::text(" three"))]);
    editor.run();
    assert_eq!(text(&editor), "one two three");

    let block = editor.block_id().expect("the editor has a block");
    editor.host().undo(block);
    editor.run();
    assert_eq!(
        text(&editor),
        "one two",
        "the editor ended a group before the paste, so it is a step of its own"
    );
    editor.host().undo(block);
    editor.run();
    assert_eq!(text(&editor), "one");
}
