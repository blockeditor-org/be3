use super::*;
use crate::geometry::line_ends;
use block_editor_beui::beui::{Pos2, Vec2};

#[test]
fn dragging_an_end_of_a_selected_line_moves_only_that_end() {
    let mut line = entity(Uuid::from_u128(1));
    line.kind = CanvasEntityKind::Line;
    line.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(180.0, 1.0), 0.0);
    line.style.line_width = 2.0;
    let mut marker = card();
    marker.id = Uuid::from_u128(2);
    marker.style.line_width = 0.0;
    marker.transform.center = CanvasPoint::new(0.0, 200.0);
    let mut editor = editor(&[line.clone(), marker.clone()]);
    let anchor = editor.rect_of(&format!("infinite-canvas.entity.{}", marker.id));
    let scale = anchor.width() / 184.0;
    let at = |x: f32, y: f32| -> Pos2 { anchor.center() + Vec2::new(x, y - 200.0) * scale };

    editor.click_at(at(0.0, 0.0));
    editor.run();
    assert_eq!(editor.label("infinite-canvas.selection"), "Line selected");
    editor.snapshot("a_selected_line_shows_handles_at_its_ends");

    editor.drag(at(92.0, 1.0), at(92.0, 61.0));
    editor.run();

    let moved = entities(&editor)
        .into_iter()
        .find(|entity| entity.id == line.id)
        .expect("the line is still on the canvas");
    assert_eq!(moved.kind, CanvasEntityKind::Line);
    let [start, end] = line_ends(&moved);
    assert!(
        (start.x + 90.0).abs() < 0.5 && start.y.abs() < 0.5,
        "the far end stays where it was, got {start:?}"
    );
    assert!(
        (end.x - 90.0).abs() < 0.5 && (end.y - 60.0).abs() < 0.5,
        "the dragged end moves by the drag, got {end:?}"
    );
}
