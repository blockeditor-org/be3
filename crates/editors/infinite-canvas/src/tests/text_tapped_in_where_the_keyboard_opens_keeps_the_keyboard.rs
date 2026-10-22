use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn text_tapped_in_where_the_keyboard_opens_keeps_the_keyboard() {
    let mut editor = phone(&[]);
    editor.click("infinite-canvas.dock.tool.Text");
    editor.run();
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let at = block_editor_beui::beui::pos2(canvas.center().x, canvas.bottom() - 160.0);
    editor.finger(1, TouchPhase::Start, at);
    editor.run();
    editor.finger(1, TouchPhase::End, at);
    editor.run();
    let keyboard =
        |editor: &BeuiTest<CanvasApp>| editor.window().ime.as_ref().map(|area| area.keyboard);
    let asked = keyboard(&editor);
    assert!(
        asked.is_some_and(|keyboard| keyboard.shown()),
        "the new text asks for the keyboard"
    );
    let [text] = entities(&editor)
        .try_into()
        .expect("the tap writes one text");
    let field = format!("infinite-canvas.text.{}", text.id);

    editor.set_size(Vec2::new(390.0, 360.0));
    for frame in 0..30 {
        editor.step(Vec::new());
        assert_eq!(keyboard(&editor), asked, "frame {frame}");
    }
    assert!(
        !editor
            .rect_of("infinite-canvas.canvas")
            .intersects(editor.rect_of(&field)),
        "the keyboard covers the text"
    );

    editor.set_size(Vec2::new(390.0, 760.0));
    for frame in 0..30 {
        editor.step(Vec::new());
        assert_eq!(keyboard(&editor), asked, "frame {frame} after the keyboard");
    }
    assert_eq!(
        entities(&editor).len(),
        1,
        "the text is still being written"
    );
}
