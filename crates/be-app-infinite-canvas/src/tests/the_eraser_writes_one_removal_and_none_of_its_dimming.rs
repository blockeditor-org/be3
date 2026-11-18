use super::*;
use be_sdk_beui::beui::{Key, Modifiers, Vec2};

#[test]
fn the_eraser_writes_one_removal_and_none_of_its_dimming() {
    let shape = placed(1, CanvasShape::Rectangle, CanvasPoint::default());
    let mut editor = vector_editor(std::slice::from_ref(&shape));
    let at = editor
        .rect_of(&format!("infinite-canvas.entity.{}", shape.id))
        .center();
    editor.store().take_written(None);

    editor.key(Key::E, Modifiers::NONE);
    editor.down(at - Vec2::new(120.0, 0.0), PointerButton::Primary);
    for step in 1..=6 {
        editor.hover(at - Vec2::new(120.0, 0.0) + Vec2::new(40.0 * step as f32, 0.0));
    }
    assert!(
        entities(&editor)[0].style.opacity < shape.style.opacity,
        "the dimming previews here"
    );
    assert!(
        editor.store().take_written(None).is_empty(),
        "nothing of an erase is written before release"
    );
    editor.up(at + Vec2::new(120.0, 0.0), PointerButton::Primary);

    assert!(entities(&editor).is_empty());
    assert_eq!(
        editor.store().take_written(None).len(),
        1,
        "the release writes the removal alone"
    );
    undo(&mut editor);
    assert_eq!(entities(&editor), vec![shape], "one undo brings it back undimmed");
}
