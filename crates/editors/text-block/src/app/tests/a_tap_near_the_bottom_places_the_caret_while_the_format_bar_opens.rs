use beui::{Pos2, Vec2};
use block_editor_beui::beui::TouchPhase;
use block_editor_beui::{EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use block_ui_test::prelude::*;
use uuid::Uuid;

use super::text;
use crate::app::TextApp;
use text_block_types::TextBlock;

#[test]
fn a_tap_near_the_bottom_places_the_caret_while_the_format_bar_opens() {
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = OwnedEditor::new(host, Uuid::new_v4());
    let mut editor = BeuiTest::<TextApp>::new(editor)
        .with_size(Vec2::new(390.0, 760.0))
        .on_phone();
    editor.hold(
        None,
        TextBlock::of(format!("top{}", "\n".repeat(60)).as_str()),
    );
    editor.run();
    assert!(!editor.shown("text.format.bold"));

    let surface = editor.rect_of("text.surface");
    let near_bottom = Pos2::new(surface.center().x, surface.max.y - 8.0);
    editor.fingers(TouchPhase::Start, &[(1, near_bottom)]);
    assert!(editor.shown("text.format.bold"));
    assert!(
        !editor.rect_of("text.surface").contains(near_bottom),
        "the bar covers where the finger went down"
    );
    editor.fingers(TouchPhase::End, &[(1, near_bottom)]);

    editor.type_text("x");
    let lines = text(&editor);
    let line = lines
        .lines()
        .position(|line| line.contains('x'))
        .expect("the letter was typed");
    assert!(line > 10, "typed on line {line}: {lines:?}");
}
