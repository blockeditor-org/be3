use be_sdk_test::BeuiTest;
use beui::{Pos2, Vec2};
use uuid::Uuid;

use crate::root::TextRoot;
use be_type_text::TextBlock;

#[test]
fn shrinking_the_view_keeps_the_caret_in_sight_and_tells_the_input_method_where_it_is() {
    let mut editor = BeuiTest::<TextRoot>::editing(Uuid::new_v4())
        .with_size(Vec2::new(390.0, 760.0))
        .on_phone();
    let lines: String = (1..=200).map(|line| format!("Line {line}\n")).collect();
    editor.hold(None, TextBlock::of(lines.as_str()));
    editor.run();

    let surface = editor.rect_of("text.surface");
    editor.tap(Pos2::new(surface.center().x, surface.max.y - 120.0));
    editor.run();
    let caret = |editor: &BeuiTest<TextRoot>| {
        editor
            .window()
            .ime
            .as_ref()
            .expect("the focused text asks for an input method")
            .cursor
    };
    let before = caret(&editor);
    assert!(
        editor.rect_of("text.surface").contains_rect(before),
        "the tapped caret shows: {before:?}"
    );

    editor.set_size(Vec2::new(390.0, 400.0));
    editor.run();
    let surface = editor.rect_of("text.surface");
    let after = caret(&editor);
    assert!(
        after.height() <= before.height() + 1.0,
        "the input method is told the caret, not the whole text: {after:?}"
    );
    assert!(
        surface.contains_rect(after),
        "the caret is scrolled into the shorter view: {after:?} in {surface:?}"
    );
    editor.snapshot("the_caret_stays_in_sight_when_the_keyboard_comes_up");
}
