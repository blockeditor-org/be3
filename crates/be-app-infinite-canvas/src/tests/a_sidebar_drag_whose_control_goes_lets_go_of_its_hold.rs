use super::*;
use be_sdk_beui::beui::Vec2;

#[test]
fn a_sidebar_drag_whose_control_goes_lets_go_of_its_hold() {
    let shape = |id: u128, x: f32| {
        let mut shape = entity(Uuid::from_u128(id));
        shape.transform =
            CanvasTransform::new(CanvasPoint::new(x, 0.0), CanvasPoint::new(120.0, 80.0), 0.0);
        shape.style.fill = CanvasFill::Solid;
        shape
    };
    let (first, second) = (shape(1, -150.0), shape(2, 150.0));
    let shapes = [first.clone(), second.clone()];
    let mut dragger = vector_editor(&shapes);
    dragger.click(format!("infinite-canvas.entity.{}", first.id));
    dragger.reveal("infinite-canvas.opacity");
    let slider = dragger.rect_of("infinite-canvas.opacity");
    let from = slider.center() + Vec2::new(slider.width() / 2.0 - 2.0, 0.0);
    dragger.down(from, PointerButton::Primary);
    dragger.hover(slider.center());
    dragger.store().take_written(None);

    // A peer removes the shape the slider is dragging.
    apply(
        &mut dragger,
        InfiniteCanvasOperation::Remove {
            ids: vec![first.id],
        },
    );
    assert!(
        dragger
            .document()
            .find_test_id("infinite-canvas.opacity")
            .is_none(),
        "the slider goes with the shape it was dragging"
    );
    dragger.up(slider.center(), PointerButton::Primary);

    let item = format!("infinite-canvas.entity.{}", second.id);
    let at = dragger.rect_of(&item).center();
    dragger.pen(PenPhase::Down(PointerButton::Primary), at, 0.5);
    dragger.pen(PenPhase::Move, at + Vec2::new(40.0, 0.0), 0.5);
    dragger.pen(
        PenPhase::Up(PointerButton::Primary),
        at + Vec2::new(40.0, 0.0),
        0.0,
    );
    assert!(
        !dragger.store().take_written(None).is_empty(),
        "the drag the slider left ended with it, so the next move is written"
    );
}
