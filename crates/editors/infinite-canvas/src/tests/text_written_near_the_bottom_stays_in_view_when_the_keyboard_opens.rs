use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2, pos2};

#[test]
fn text_written_near_the_bottom_stays_in_view_when_the_keyboard_opens() {
    let mut editor = phone(&[]);
    editor.reveal("infinite-canvas.dock.tool.Text");
    editor.click("infinite-canvas.dock.tool.Text");
    editor.run();
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let at = pos2(canvas.center().x - 100.0, canvas.bottom() - 160.0);
    editor.fingers(TouchPhase::Start, &[(1, at)]);
    editor.run();
    editor.fingers(TouchPhase::End, &[(1, at)]);
    editor.run();
    let [text] = entities(&editor)
        .try_into()
        .expect("the tap writes one text");
    let field = format!("infinite-canvas.text.{}", text.id);

    editor.set_size(Vec2::new(390.0, 360.0));
    editor.settle();
    let shown = editor.rect_of(&field);
    let canvas = editor.rect_of("infinite-canvas.canvas");
    assert!(
        canvas.contains_rect(shown),
        "the text at {shown:?} is inside the canvas left above the keyboard, {canvas:?}"
    );
    for chrome in ["infinite-canvas.dock", "infinite-canvas.zoom-pill"] {
        let covers = editor.rect_of(chrome);
        assert!(
            !covers.intersects(shown),
            "the text at {shown:?} is clear of {chrome} at {covers:?}"
        );
    }
    editor.snapshot("text_written_near_the_bottom_stays_in_view_when_the_keyboard_opens");
    editor.type_text("Hi");
    editor.run();
    assert_eq!(
        entities(&editor)[0].text.to_str_lossy(),
        "Hi",
        "the text is still being written"
    );
}
