use super::*;
use block_editor_beui::beui::{Key, Modifiers, Vec2};

fn filled(id: u128, x: f32) -> CanvasEntity {
    let mut entity = card();
    entity.id = Uuid::from_u128(id);
    entity.transform =
        CanvasTransform::new(CanvasPoint::new(x, 0.0), CanvasPoint::new(80.0, 60.0), 0.0);
    entity
}

#[test]
fn the_eraser_finds_shapes_where_they_have_moved_to() {
    let (left, right) = (filled(1, -160.0), filled(2, 160.0));
    let mut editor = vector_editor(&[left.clone(), right.clone()]);
    let mut moved = left.clone();
    moved.transform.center = CanvasPoint::new(-160.0, 150.0);
    apply(
        &mut editor,
        InfiniteCanvasOperation::Update {
            entities: vec![moved],
        },
    );
    editor.run();
    let at = editor
        .rect_of(&format!("infinite-canvas.entity.{}", left.id))
        .center();

    editor.key(Key::E, Modifiers::NONE);
    editor.down(at, PointerButton::Primary);
    for step in 1..=3 {
        editor.next_frame();
        editor.hover(at + Vec2::new(step as f32 * 4.0, 0.0));
    }
    editor.run();
    editor.up(at + Vec2::new(12.0, 0.0), PointerButton::Primary);

    let kept: Vec<Uuid> = entities(&editor).iter().map(|entity| entity.id).collect();
    assert_eq!(
        kept,
        [right.id],
        "the moved shape is erased where it now is"
    );
}
