use super::*;
use block_editor_beui::beui::{TouchPhase, pos2};
use std::time::Duration;

#[test]
fn a_drag_in_a_label_being_edited_selects_it_and_leaves_the_shape_in_place() {
    let mut shape = placed(1, CanvasShape::Rectangle, CanvasPoint::default());
    shape.transform.size = CanvasPoint::new(240.0, 80.0);
    shape.style.fill = CanvasFill::Solid;
    shape.text = "Hello world".into();
    let mut editor = editor(std::slice::from_ref(&shape));
    let field = format!("infinite-canvas.text.{}", shape.id);

    editor.double_click(format!("infinite-canvas.entity.{}", shape.id));
    let rect = editor.rect_of(&field);
    let y = rect.center().y;
    let (left, right) = (pos2(rect.left() + 2.0, y), pos2(rect.right() - 2.0, y));

    editor.advance(Duration::from_secs(1));
    editor.run();
    editor.drag(left, right);
    editor.pen(PenPhase::Down(PointerButton::Primary), right, 0.5);
    editor.pen(PenPhase::Move, left, 0.5);
    editor.run();
    editor.pen(PenPhase::Up(PointerButton::Primary), left, 0.5);
    editor.fingers(TouchPhase::Start, &[(1, left)]);
    editor.fingers(TouchPhase::Move, &[(1, right)]);
    editor.fingers(TouchPhase::End, &[(1, right)]);
    assert_eq!(
        entities(&editor)[0].transform,
        shape.transform,
        "no drag in the label being edited moves its shape, by mouse, pen or finger"
    );

    editor.advance(Duration::from_secs(1));
    editor.run();
    editor.drag(left, right);
    editor.type_text("Bye");
    assert_eq!(
        entities(&editor)[0].text.to_str_lossy(),
        "Bye",
        "the drag selected the label it crossed"
    );
}
