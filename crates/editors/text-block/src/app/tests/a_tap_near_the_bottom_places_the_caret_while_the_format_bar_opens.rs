use beui::{Pos2, Vec2};
use block_editor_beui::be_block::TextContent;
use block_editor_beui::{Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use super::text;
use crate::app::TextApp;

#[test]
fn a_tap_near_the_bottom_places_the_caret_while_the_format_bar_opens() {
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host, Uuid::new_v4());
    let mut editor = BeuiTest::<TextApp>::new(editor)
        .with_size(Vec2::new(390.0, 760.0))
        .on_phone();
    editor.hold(None, TextContent::from(format!("top{}", "\n".repeat(60)).as_str()));
    editor.run();
    assert!(!editor.shown("text.format.bold"));

    let surface = editor.rect_of("text.surface");
    let near_bottom = Pos2::new(surface.center().x, surface.max.y - 8.0);
    editor.touch_start(near_bottom);
    editor.run();
    assert!(editor.shown("text.format.bold"));
    assert!(
        !editor.rect_of("text.surface").contains(near_bottom),
        "the bar covers where the finger went down"
    );
    editor.touch_end(near_bottom);
    editor.run();

    editor.text("x");
    editor.run();
    let lines = text(&editor);
    let line = lines
        .lines()
        .position(|line| line.contains('x'))
        .expect("the letter was typed");
    assert!(line > 10, "typed on line {line}: {lines:?}");
}
