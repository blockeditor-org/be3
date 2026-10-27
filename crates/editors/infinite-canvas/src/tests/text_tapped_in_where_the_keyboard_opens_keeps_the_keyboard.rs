use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn text_tapped_in_where_the_keyboard_opens_keeps_the_keyboard() {
    let mut editor = phone(&[]);
    editor.reveal("infinite-canvas.dock.tool.Text");
    editor.click("infinite-canvas.dock.tool.Text");
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let at = block_editor_beui::beui::pos2(canvas.center().x, canvas.bottom() - 160.0);
    editor.fingers(TouchPhase::Start, &[(1, at)]);
    editor.fingers(TouchPhase::End, &[(1, at)]);
    let keyboard =
        |editor: &BeuiTest<CanvasApp>| editor.window().ime.as_ref().map(|area| area.keyboard);
    let asked = keyboard(&editor);
    assert!(
        asked.is_some_and(|keyboard| keyboard.shown()),
        "the new text asks for the keyboard"
    );
    assert_eq!(entities(&editor).len(), 1, "the tap writes one text");

    editor.set_size(Vec2::new(390.0, 360.0));
    for frame in 0..30 {
        editor.step(Vec::new());
        assert_eq!(keyboard(&editor), asked, "frame {frame}");
    }

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
