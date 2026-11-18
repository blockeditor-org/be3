use super::*;
use be_sdk_beui::beui::{Key, Modifiers, Vec2};

#[test]
fn escape_mid_resize_puts_the_size_back_and_writes_nothing() {
    let mut rectangle = card();
    rectangle.style.stroke_width = 0.0;
    let mut editor = editor(std::slice::from_ref(&rectangle));
    let shape = format!("infinite-canvas.entity.{}", rectangle.id);
    let before = editor.rect_of(&shape);
    let scale = before.width() / 184.0;
    editor.click(&shape);
    editor.store().take_written(None);

    let corner = before.center() + Vec2::new(90.0, 60.0) * scale;
    let from = corner + Vec2::new(-4.0, -4.0);
    editor.down(from, PointerButton::Primary);
    editor.hover(from + Vec2::new(15.0, 10.0));
    editor.hover(from + Vec2::new(30.0, 20.0));
    assert_ne!(
        entities(&editor)[0].transform.size,
        rectangle.transform.size,
        "the resize previews"
    );

    editor.key(Key::Escape, Modifiers::NONE);
    editor.up(from + Vec2::new(30.0, 20.0), PointerButton::Primary);
    assert_eq!(entities(&editor)[0].transform, rectangle.transform);
    assert!(
        editor.store().take_written(None).is_empty(),
        "a resize called off leaves no trace"
    );
}
