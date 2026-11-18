use super::*;
use beui::{PointerButton, TouchPhase};

#[test]
fn a_thumb_lifted_at_the_press_follows_the_mouse_while_a_finger_carries_the_other() {
    let mut test = alone(WIDE, Page::Dragging);
    reveal(&mut test, "demo.thumb.1");
    let first = test.rect_of("demo.thumb.0");
    let second = test.rect_of("demo.thumb.1");

    test.down(first.center(), PointerButton::Primary);
    test.frame(vec![Input::PointerMoved(
        first.center() + Vec2::new(2.0, 0.0),
    )]);
    assert_eq!(
        test.rect_of("demo.thumb.0"),
        first.translate(Vec2::new(2.0, 0.0)),
        "a thumb lifted at the mouse's press follows it from the first point"
    );
    test.fingers(TouchPhase::Start, &[(1, second.center())]);
    test.fingers(
        TouchPhase::Move,
        &[(1, second.center() + Vec2::new(120.0, 0.0))],
    );
    test.frame(vec![Input::PointerMoved(
        first.center() + Vec2::new(60.0, 0.0),
    )]);
    test.settle();
    assert_eq!(
        test.rect_of("demo.thumb.0"),
        first.translate(Vec2::new(60.0, 0.0)),
        "the mouse carries the first"
    );
    assert_eq!(
        test.rect_of("demo.thumb.1"),
        second.translate(Vec2::new(120.0, 0.0)),
        "while a finger carries the second"
    );
    test.snapshot("a_thumb_lifted_at_the_press_follows_the_mouse_while_a_finger_carries_the_other");
    test.fingers(
        TouchPhase::End,
        &[(1, second.center() + Vec2::new(120.0, 0.0))],
    );
    test.up(
        first.center() + Vec2::new(60.0, 0.0),
        PointerButton::Primary,
    );
    test.settle();
    assert_eq!(
        test.rect_of("demo.thumb.0"),
        first.translate(Vec2::new(60.0, 0.0)),
        "each stays where it was let go"
    );
}
