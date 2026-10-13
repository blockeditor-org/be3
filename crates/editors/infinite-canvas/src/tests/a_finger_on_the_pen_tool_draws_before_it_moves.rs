use super::*;
use block_editor_beui::beui::TouchPhase;

#[test]
fn a_finger_on_the_pen_tool_draws_before_it_moves() {
    let mut editor = phone(&[]);

    editor.click("infinite-canvas.dock.tool.Pen");
    editor.run();
    let at = editor.rect_of("infinite-canvas.canvas").center();
    editor.finger(1, TouchPhase::Start, at);
    editor.run();
    editor.snapshot("a_finger_on_the_pen_tool_draws_before_it_moves");
    editor.finger(1, TouchPhase::End, at);
    editor.run();

    let entities = entities(&editor);
    assert_eq!(entities.len(), 1, "the tap adds one stroke");
    assert_eq!(
        entities[0].kind,
        CanvasEntityKind::Pen {
            points: vec![CanvasPoint::default()]
        },
        "a tap is kept as a dot"
    );
}
