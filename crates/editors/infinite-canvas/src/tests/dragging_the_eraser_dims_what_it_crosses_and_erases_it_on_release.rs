use super::*;
use block_editor_beui::beui::{Key, Modifiers, Pos2, Vec2};

fn filled(id: u128, x: f32) -> CanvasEntity {
    let mut entity = card();
    entity.id = Uuid::from_u128(id);
    entity.transform =
        CanvasTransform::new(CanvasPoint::new(x, 0.0), CanvasPoint::new(80.0, 60.0), 0.0);
    entity
}

#[test]
fn dragging_the_eraser_dims_what_it_crosses_and_erases_it_on_release() {
    let left = filled(1, -160.0);
    let mut middle = filled(2, 0.0);
    middle.locked = true;
    let right = filled(3, 160.0);
    let around = frame(4, CanvasPoint::default(), CanvasPoint::new(460.0, 200.0));
    let mut editor = vector_editor(&[around.clone(), left.clone(), middle.clone(), right.clone()]);
    let center = |editor: &BeuiTest<CanvasApp>, entity: &CanvasEntity| {
        editor
            .rect_of(&format!("infinite-canvas.entity.{}", entity.id))
            .center()
    };
    let walk = |editor: &mut BeuiTest<CanvasApp>, from: Pos2, to: Pos2| {
        for step in 1..=6 {
            editor.next_frame();
            editor.hover(from + (to - from) * (step as f32 / 6.0));
        }
        editor.run();
    };
    let (first, locked, last) = (
        center(&editor, &left),
        center(&editor, &middle),
        center(&editor, &right),
    );
    let bounds = editor.rect_of(&format!("infinite-canvas.entity.{}", around.id));
    let outside = Pos2::new(bounds.left() - 30.0, first.y);

    editor.key(Key::E, Modifiers::NONE);
    editor.run();
    editor.down(outside, PointerButton::Primary);
    walk(&mut editor, outside, locked);
    assert_eq!(
        entities(&editor).len(),
        4,
        "nothing is erased before release"
    );
    assert!(
        entities(&editor)[1].style.opacity < left.style.opacity,
        "what the eraser crossed is dimmed in the document, so peers see it too"
    );
    editor.snapshot("dragging_the_eraser_dims_what_it_crosses_and_erases_it_on_release");

    walk(&mut editor, locked, last + Vec2::new(50.0, 0.0));
    editor.hold_modifiers(Modifiers {
        alt: true,
        ..Modifiers::NONE
    });
    walk(
        &mut editor,
        last + Vec2::new(50.0, 0.0),
        last - Vec2::new(50.0, 0.0),
    );
    editor.hold_modifiers(Modifiers::NONE);
    editor.up(last - Vec2::new(50.0, 0.0), PointerButton::Primary);
    editor.run();

    assert_eq!(
        entities(&editor),
        vec![around.clone(), middle.clone(), right.clone()],
        "the eraser takes what it crossed, leaves frames and locked shapes, and Alt restores"
    );

    editor.down(last + Vec2::new(0.0, 50.0), PointerButton::Primary);
    walk(
        &mut editor,
        last + Vec2::new(0.0, 50.0),
        last - Vec2::new(0.0, 50.0),
    );
    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();
    editor.up(last - Vec2::new(0.0, 50.0), PointerButton::Primary);
    editor.run();
    assert_eq!(
        entities(&editor),
        vec![around.clone(), middle.clone(), right.clone()],
        "Escape calls the erase off and puts back what it dimmed"
    );

    undo(&mut editor);
    assert_eq!(
        entities(&editor),
        vec![around, left, middle, right],
        "one undo takes back the whole erase, and the one called off left nothing to undo"
    );
}
