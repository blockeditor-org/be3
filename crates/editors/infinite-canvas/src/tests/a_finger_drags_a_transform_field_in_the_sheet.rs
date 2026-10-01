use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn a_finger_drags_a_transform_field_in_the_sheet() {
    let rectangle = card();
    let mut editor = phone(std::slice::from_ref(&rectangle));
    let shape = format!("infinite-canvas.entity.{}", rectangle.id);
    let at = editor.rect_of(&shape).center();
    editor.finger(1, TouchPhase::Start, at);
    editor.run();
    editor.finger(1, TouchPhase::End, at);
    editor.run();
    editor.click("infinite-canvas.inspect");
    editor.run();
    let handle = editor.rect_of("sheet.handle").center();
    editor.drag(handle, handle - Vec2::new(0.0, 400.0));
    editor.settle_until("the sheet to come to rest", |editor| {
        !editor.wants_another_frame()
    });

    let held = entities(&editor)[0].transform;
    let from = editor.point_of("infinite-canvas.transform.x");
    editor.finger(1, TouchPhase::Start, from);
    editor.run();
    editor.finger(1, TouchPhase::Move, from + Vec2::new(20.0, 1.0));
    editor.run();
    editor.finger(1, TouchPhase::Move, from + Vec2::new(40.0, 2.0));
    editor.run();
    editor.finger(1, TouchPhase::End, from + Vec2::new(40.0, 2.0));
    editor.run();

    let moved = entities(&editor)[0].transform;
    assert!(
        moved.center.x > held.center.x,
        "a sideways finger drag on the field edits x, got {moved:?}"
    );
}
