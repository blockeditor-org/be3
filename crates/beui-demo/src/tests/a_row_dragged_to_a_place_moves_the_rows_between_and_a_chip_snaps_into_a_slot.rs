use super::*;
use beui::PointerButton;
use std::time::Duration;

#[test]
fn a_row_dragged_to_a_place_moves_the_rows_between_and_a_chip_snaps_into_a_slot() {
    let mut test = alone(WIDE, Page::Dragging);
    reveal(&mut test, "demo.places.5");
    let grab = test.rect_of("demo.places.0").center();
    let below = test.rect_of("demo.places.2").center();
    test.down(grab, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(grab + Vec2::new(0.0, 6.0))]);
    test.frame(vec![Input::PointerMoved(below + Vec2::new(0.0, 4.0))]);
    test.settle();
    test.record();
    test.up(below + Vec2::new(0.0, 4.0), PointerButton::Primary);
    test.settle();
    test.record();

    reveal(&mut test, "demo.snap.chip");
    let chip = test.rect_of("demo.snap.chip").center();
    test.down(chip, PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(chip + Vec2::new(6.0, 0.0))]);
    test.frame(vec![Input::PointerMoved(chip + Vec2::new(110.0, -70.0))]);
    test.advance(Duration::from_millis(200));
    test.settle();
    test.record();
    test.up(chip + Vec2::new(110.0, -70.0), PointerButton::Primary);
    test.settle();
    test.record();

    let snapped = test.rect_of("demo.snap.chip");
    let away = Vec2::new(-200.0, 110.0);
    test.down(snapped.center(), PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(
        snapped.center() + Vec2::new(-6.0, 0.0),
    )]);
    test.frame(vec![Input::PointerMoved(snapped.center() + away)]);
    test.settle();
    test.up(snapped.center() + away, PointerButton::Primary);
    test.settle();
    assert_eq!(
        test.rect_of("demo.snap.chip"),
        snapped.translate(away),
        "a chip let go far from every slot stays where it was let go"
    );
    test.snapshot("a_row_dragged_to_a_place_moves_the_rows_between_and_a_chip_snaps_into_a_slot");
}
