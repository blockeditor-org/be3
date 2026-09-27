use super::*;
use block_editor_beui::beui::{TouchPhase, Vec2};

#[test]
fn the_select_tool_box_selects_under_a_finger() {
    let rectangle = card();
    let mut editor = editor(std::slice::from_ref(&rectangle));
    let shape = editor.rect_of(&format!("infinite-canvas.entity.{}", rectangle.id));

    editor.click("infinite-canvas.tool.Select");
    editor.run();
    let from = shape.left_top() - Vec2::new(30.0, 30.0);
    let to = shape.right_bottom() + Vec2::new(30.0, 30.0);
    editor.finger(1, TouchPhase::Start, from);
    editor.run();
    editor.finger(1, TouchPhase::Move, from + (to - from) * 0.5);
    editor.run();
    editor.finger(1, TouchPhase::Move, to);
    editor.run();
    editor.finger(1, TouchPhase::End, to);
    editor.run();

    assert_eq!(
        editor.rect_of(&format!("infinite-canvas.entity.{}", rectangle.id)),
        shape,
        "the select tool leaves the view where it was"
    );
    editor.click("infinite-canvas.delete");
    editor.run();
    assert!(
        entities(&editor).is_empty(),
        "the box selected the rectangle"
    );
}
