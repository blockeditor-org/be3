use super::*;
use block_editor_beui::beui::{TouchPhase, pos2};

#[test]
fn a_text_tapped_in_starts_at_the_tap() {
    let mut editor = phone(&[]);
    editor.click("infinite-canvas.pill.zoom-in");
    editor.settle();
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let tapped = |editor: &mut BeuiTest<CanvasApp>, x: f32| {
        editor.reveal("infinite-canvas.dock.tool.Text");
        editor.click("infinite-canvas.dock.tool.Text");
        let at = pos2(x, canvas.center().y);
        editor.fingers(TouchPhase::Start, &[(1, at)]);
        editor.fingers(TouchPhase::End, &[(1, at)]);
        editor.settle();
        let text = entities(editor)
            .into_iter()
            .last()
            .expect("the tap writes a text");
        editor.rect_of(&format!("infinite-canvas.text.{}", text.id))
    };

    let x = canvas.left() + 12.0;
    let field = tapped(&mut editor, x);
    assert!(
        field.left() <= x && x - field.left() < 20.0,
        "the text at {field:?} starts at the tap, {x}"
    );
    assert!(
        canvas.contains_rect(field),
        "the text at {field:?} is in view, in {canvas:?}"
    );

    editor.snapshot("a_text_tapped_in_starts_at_the_tap");
    let field = tapped(&mut editor, canvas.right() - 12.0);
    assert!(
        canvas.contains_rect(field),
        "the text tapped in at the right edge is moved into view, at {field:?} in {canvas:?}"
    );
}
