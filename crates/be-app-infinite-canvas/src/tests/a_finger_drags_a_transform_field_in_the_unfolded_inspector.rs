use super::*;
use be_sdk_beui::beui::{TouchPhase, Vec2};

#[test]
fn a_finger_drags_a_transform_field_in_the_unfolded_inspector() {
    let rectangle = card();
    let mut editor = vector_phone(std::slice::from_ref(&rectangle));
    let shape = format!("infinite-canvas.entity.{}", rectangle.id);
    let at = editor.rect_of(&shape).center();
    editor.fingers(TouchPhase::Start, &[(1, at)]);
    editor.run();
    editor.fingers(TouchPhase::End, &[(1, at)]);
    editor.run();
    editor.tap("infinite-canvas.inspect");
    editor.run();
    editor.settle();
    editor.reveal("infinite-canvas.transform.x");

    let held = entities(&editor)[0].transform;
    let from = editor.point_of("infinite-canvas.transform.x");
    editor.fingers(TouchPhase::Start, &[(1, from)]);
    editor.run();
    editor.fingers(TouchPhase::Move, &[(1, from + Vec2::new(20.0, 1.0))]);
    editor.run();
    editor.fingers(TouchPhase::Move, &[(1, from + Vec2::new(40.0, 2.0))]);
    editor.run();
    editor.fingers(TouchPhase::End, &[(1, from + Vec2::new(40.0, 2.0))]);
    editor.run();

    let moved = entities(&editor)[0].transform;
    assert!(
        moved.center.x > held.center.x,
        "a sideways finger drag on the field edits x, got {moved:?}"
    );
}
