use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn drawing_with_a_finger_adds_a_rectangle() {
    let mut editor = phone(&[]);

    editor.click("infinite-canvas.dock.tool.Rectangle");
    editor.run();
    let from = editor.rect_of("infinite-canvas.canvas").center();
    editor.fingers(TouchPhase::Start, &[(1, from)]);
    editor.run();
    editor.fingers(TouchPhase::Move, &[(1, from + Vec2::new(10.0, 60.0))]);
    editor.run();
    editor.fingers(TouchPhase::Move, &[(1, from + Vec2::new(40.0, 150.0))]);
    editor.run();
    editor.fingers(TouchPhase::End, &[(1, from + Vec2::new(40.0, 150.0))]);
    editor.run();

    let entities = entities(&editor);
    assert_eq!(entities.len(), 1, "the drag adds one entity");
    assert_eq!(entities[0].shape, CanvasShape::Rectangle);
    assert!(
        entities[0].transform.size.y > entities[0].transform.size.x * 2.0,
        "a mostly vertical drag is drawn, not taken for a scroll"
    );
}
