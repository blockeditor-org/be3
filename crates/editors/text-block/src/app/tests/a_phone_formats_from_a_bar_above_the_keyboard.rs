use beui::Vec2;
use block_editor_beui::be_block::TextContent;
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use super::text;
use crate::app::TextApp;

#[test]
fn a_phone_formats_from_a_bar_above_the_keyboard() {
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, Uuid::new_v4());
    let mut editor = BeuiTest::<TextApp>::new(editor)
        .with_size(Vec2::new(390.0, 760.0))
        .with_phone_bar(1);
    editor.hold(None, TextContent::from("word"));
    editor.run();

    assert!(!editor.shown("text.hex-view"));
    assert!(!editor.shown("text.format.bold"));

    editor.click("text.surface");
    editor.run();
    assert!(editor.shown("text.format.bold"));
    editor.snapshot("a_phone_formats_from_a_bar_above_the_keyboard");

    editor.click("text.format.bold");
    editor.run();
    assert!(text(&editor).contains("**"), "{:?}", text(&editor));
    assert!(
        editor.shown("text.format.bold"),
        "the bar stays while the text keeps the focus"
    );

    editor.click("text.format.done");
    editor.run();
    assert!(!editor.shown("text.format.bold"));

    editor.click("editor.more");
    editor.run();
    assert!(editor.label("editor.more.item.1").ends_with("Find and replace"));
}
