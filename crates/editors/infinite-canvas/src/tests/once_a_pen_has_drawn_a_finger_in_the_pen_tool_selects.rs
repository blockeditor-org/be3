use super::*;
use block_editor_beui::beui::{PenPhase, PointerButton, TouchPhase};

#[test]
fn once_a_pen_has_drawn_a_finger_in_the_pen_tool_selects() {
    let rectangle = card();
    let mut editor = phone(std::slice::from_ref(&rectangle));
    editor.click("infinite-canvas.dock.tool.Pen");
    editor.run();
    let shape = editor.rect_of(&format!("infinite-canvas.entity.{}", rectangle.id));

    let from = shape.left_top() - Vec2::new(60.0, 60.0);
    editor.pen(PenPhase::Move, from, 0.0);
    editor.next_frame();
    editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    editor.next_frame();
    editor.pen(PenPhase::Move, from + Vec2::new(40.0, 0.0), 0.5);
    editor.next_frame();
    editor.pen(
        PenPhase::Up(PointerButton::Primary),
        from + Vec2::new(40.0, 0.0),
        0.0,
    );
    editor.run();
    assert_eq!(entities(&editor).len(), 2, "the pen draws");

    editor.fingers(TouchPhase::Start, &[(1, shape.center())]);
    editor.run();
    editor.fingers(TouchPhase::End, &[(1, shape.center())]);
    editor.run();
    assert_eq!(entities(&editor).len(), 2, "a finger draws nothing");
    assert!(
        editor.shown("infinite-canvas.selection-bar"),
        "the finger selects what it taps"
    );
    assert!(
        editor.shown("infinite-canvas.dock.tool.Pen"),
        "and the pen tool stays chosen"
    );

    let empty = shape.right_bottom() + Vec2::new(40.0, 40.0);
    editor.fingers(TouchPhase::Start, &[(1, empty)]);
    editor.run();
    editor.fingers(TouchPhase::Move, &[(1, empty + Vec2::new(60.0, 30.0))]);
    editor.run();
    editor.fingers(TouchPhase::End, &[(1, empty + Vec2::new(60.0, 30.0))]);
    editor.run();
    assert_eq!(
        entities(&editor).len(),
        2,
        "a finger dragged across empty canvas draws nothing either"
    );

    editor.pen(PenPhase::Down(PointerButton::Primary), empty, 0.5);
    editor.next_frame();
    editor.pen(PenPhase::Up(PointerButton::Primary), empty, 0.0);
    editor.run();
    assert_eq!(entities(&editor).len(), 3, "and the pen still draws");
}
