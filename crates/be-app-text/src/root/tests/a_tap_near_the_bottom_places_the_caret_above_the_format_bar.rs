use be_sdk_beui::beui::TouchPhase;
use be_sdk_test::BeuiTest;
use be_sdk_test::prelude::*;
use beui::{Pos2, Vec2};
use uuid::Uuid;

use super::text;
use crate::root::TextRoot;
use be_type_text::TextBlock;

#[test]
fn a_tap_near_the_bottom_places_the_caret_above_the_format_bar() {
    let mut editor = BeuiTest::<TextRoot>::editing(Uuid::new_v4())
        .with_size(Vec2::new(390.0, 760.0))
        .on_phone();
    editor.hold(
        None,
        TextBlock::of(format!("top{}", "\n".repeat(60)).as_str()),
    );
    editor.run();
    let bar = editor.rect_of("text.format");
    let surface = editor.rect_of("text.surface");
    assert!(
        bar.min.y >= surface.max.y - 0.5,
        "the bar sits below the text, not over it: bar {bar:?}, text {surface:?}"
    );

    let near_bottom = Pos2::new(surface.center().x, surface.max.y - 8.0);
    editor.fingers(TouchPhase::Start, &[(1, near_bottom)]);
    editor.fingers(TouchPhase::End, &[(1, near_bottom)]);

    editor.type_text("x");
    let lines = text(&editor);
    let line = lines
        .lines()
        .position(|line| line.contains('x'))
        .expect("the letter was typed");
    assert!(line > 10, "typed on line {line}: {lines:?}");
}
