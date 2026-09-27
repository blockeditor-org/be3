use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2, pos2};

#[test]
fn dragging_empty_canvas_with_a_finger_pans_it() {
    let rectangle = card();
    let mut editor = phone(std::slice::from_ref(&rectangle));
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let from = pos2(canvas.center().x, canvas.top() + 40.0);
    let shape = format!("infinite-canvas.entity.{}", rectangle.id);
    let before = editor.rect_of(&shape).center();

    editor.finger(1, TouchPhase::Start, from);
    editor.run();
    editor.finger(1, TouchPhase::Move, from + Vec2::new(10.0, 60.0));
    editor.run();
    editor.finger(1, TouchPhase::Move, from + Vec2::new(20.0, 120.0));
    editor.run();
    editor.finger(1, TouchPhase::End, from + Vec2::new(20.0, 120.0));
    editor.run();

    let panned = editor.rect_of(&shape).center() - before;
    assert!(
        (panned - Vec2::new(20.0, 120.0)).length() < 1.0,
        "the canvas follows the finger, moved {panned:?}"
    );
    assert_eq!(
        entities(&editor),
        vec![rectangle],
        "nothing moved or was added"
    );
    editor.snapshot("dragging_empty_canvas_with_a_finger_pans_it");
}
