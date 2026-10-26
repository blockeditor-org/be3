use super::*;
use block_editor_beui::beui::{Key, Modifiers, Vec2};

#[test]
fn a_letter_typed_into_a_field_picks_no_tool() {
    let rectangle = card();
    let mut editor = vector_editor(std::slice::from_ref(&rectangle));

    editor.click(format!("infinite-canvas.entity.{}", rectangle.id));
    editor.run();
    editor.click("infinite-canvas.transform.x");
    editor.run();
    editor.key(Key::R, Modifiers::NONE);
    editor.type_text("r");
    editor.run();
    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();

    let canvas = editor.rect_of("infinite-canvas.canvas");
    let from = canvas.min + Vec2::new(30.0, 30.0);
    editor.drag(from, from + Vec2::new(60.0, 40.0));
    editor.run();
    assert_eq!(
        entities(&editor),
        vec![rectangle],
        "R typed into a field leaves the select tool picked"
    );
}
