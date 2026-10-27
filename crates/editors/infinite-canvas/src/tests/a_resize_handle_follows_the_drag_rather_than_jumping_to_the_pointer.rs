use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn a_resize_handle_follows_the_drag_rather_than_jumping_to_the_pointer() {
    let mut rectangle = card();
    rectangle.style.stroke_width = 0.0;
    let mut editor = editor(std::slice::from_ref(&rectangle));
    let shape = format!("infinite-canvas.entity.{}", rectangle.id);
    let before = editor.rect_of(&shape);
    let scale = before.width() / 184.0;
    editor.click(&shape);

    let corner = before.center() + Vec2::new(90.0, 60.0) * scale;
    let from = corner + Vec2::new(-4.0, -4.0);
    editor.drag(from, from + Vec2::new(30.0, 20.0));

    let size = entities(&editor)[0].transform.size;
    let expected = Vec2::new(180.0 + 30.0 / scale, 120.0 + 20.0 / scale);
    assert!(
        (size.x - expected.x).abs() < 0.5 && (size.y - expected.y).abs() < 0.5,
        "the corner moved by the drag alone, to {expected:?}, not to the pointer: got {size:?}"
    );
}
