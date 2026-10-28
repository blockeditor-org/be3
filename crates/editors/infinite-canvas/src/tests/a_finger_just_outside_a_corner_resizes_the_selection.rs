use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn a_finger_just_outside_a_corner_resizes_the_selection() {
    let rectangle = card();
    let mut editor = phone(std::slice::from_ref(&rectangle));
    let shape = format!("infinite-canvas.entity.{}", rectangle.id);
    let bounds = editor.rect_of(&shape);
    editor.fingers(TouchPhase::Start, &[(1, bounds.center())]);
    editor.fingers(TouchPhase::End, &[(1, bounds.center())]);

    let from = bounds.right_bottom() + Vec2::new(8.0, 8.0);
    editor.fingers(TouchPhase::Start, &[(1, from)]);
    editor.fingers(TouchPhase::Move, &[(1, from + Vec2::new(20.0, 30.0))]);
    editor.fingers(TouchPhase::Move, &[(1, from + Vec2::new(40.0, 60.0))]);
    editor.fingers(TouchPhase::End, &[(1, from + Vec2::new(40.0, 60.0))]);

    let resized = &entities(&editor)[0];
    assert!(
        resized.transform.size.x > 180.0 && resized.transform.size.y > 120.0,
        "the corner handle is within a finger's reach, got {:?}",
        resized.transform.size
    );
}
