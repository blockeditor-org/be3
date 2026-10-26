use super::*;
use block_editor_beui::beui::{Key, Modifiers};

#[test]
fn a_multiple_selection_outlines_each_selected_entity() {
    let mut left = card();
    left.transform.center = CanvasPoint::new(-140.0, -60.0);
    let mut right = entity(Uuid::from_u128(2));
    right.transform = CanvasTransform::new(
        CanvasPoint::new(120.0, 80.0),
        CanvasPoint::new(100.0, 60.0),
        0.3,
    );
    let mut editor = editor(&[left.clone(), right]);

    editor.click(format!("infinite-canvas.entity.{}", left.id));
    editor.run();
    editor.key(
        Key::A,
        Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        },
    );
    editor.run();

    editor.snapshot("a_multiple_selection_outlines_each_selected_entity");
}
