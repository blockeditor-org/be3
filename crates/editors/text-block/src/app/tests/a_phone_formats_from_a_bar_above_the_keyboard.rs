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
        .on_phone();
    editor.hold(None, TextContent::from("word"));
    editor.run();

    assert!(!editor.shown("text.format.bold"));

    let surface = editor.point_of("text.surface");
    editor.touch_start(surface);
    editor.touch_end(surface);
    editor.run();
    assert!(editor.shown("text.format.done"));
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

    assert_eq!(
        editor.menu_entry("text.replace").label,
        "Find and replace",
        "the text area's own commands are in the frame's menu"
    );
}
