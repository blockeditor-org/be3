use super::*;
use beui::{Key, Modifiers};

fn pressed(test: &DocumentTest, id: &str) -> bool {
    let button = test
        .document()
        .find_test_id(id)
        .expect("the format bar shows its buttons");
    test.document().api::<ToggleApi>(button).checked().get()
}

#[test]
fn the_rich_text_bar_styles_a_selection_and_shows_the_style_under_the_caret() {
    let mut test = alone(WIDE, Page::RichText);
    let editor = test.rect_of("demo.rich.editor");
    select_line(&mut test, pos2(editor.left() + 40.0, editor.top() + 70.0));
    assert!(!pressed(&test, "demo.rich.strikethrough"));

    test.click("demo.rich.strikethrough");
    test.frame(Vec::new());
    assert!(pressed(&test, "demo.rich.strikethrough"));

    press_key(&mut test, Key::End);
    press_key_with(&mut test, Key::B, Modifiers::CTRL);
    test.frame(vec![Input::Text(" Typed after.".to_owned())]);
    test.frame(Vec::new());
    assert!(
        pressed(&test, "demo.rich.bold") && pressed(&test, "demo.rich.strikethrough"),
        "text typed after a toggle at the caret takes the line's style and the toggled one"
    );

    test.click("demo.rich.heading2");
    test.frame(Vec::new());
    assert!(pressed(&test, "demo.rich.heading2"));
    test.snapshot("rich_text_styled");
}
