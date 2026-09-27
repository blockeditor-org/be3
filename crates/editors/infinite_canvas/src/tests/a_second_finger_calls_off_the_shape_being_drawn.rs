use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn a_second_finger_calls_off_the_shape_being_drawn() {
    let mut editor = phone(&[]);

    editor.click("infinite-canvas.tool.Rectangle");
    editor.run();
    let from = editor.rect_of("infinite-canvas.canvas").center();
    editor.finger(1, TouchPhase::Start, from);
    editor.run();
    editor.finger(1, TouchPhase::Move, from + Vec2::new(80.0, 80.0));
    editor.run();
    editor.finger(2, TouchPhase::Start, from + Vec2::new(-60.0, -60.0));
    editor.run();
    editor.finger(2, TouchPhase::Move, from + Vec2::new(-90.0, -90.0));
    editor.run();
    editor.finger(1, TouchPhase::End, from + Vec2::new(80.0, 80.0));
    editor.run();
    editor.finger(2, TouchPhase::End, from + Vec2::new(-90.0, -90.0));
    editor.run();

    assert!(
        entities(&editor).is_empty(),
        "pinching partway through a drag draws nothing"
    );
}
