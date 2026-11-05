use super::*;
use be_sdk_beui::beui::{PointerButton, Vec2};
use std::time::Duration;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);

#[test]
fn a_slide_dragged_along_a_wide_filmstrip_passes_the_slide_beside_it() {
    let (test, _editor) = editor(3);
    let mut test = test.with_size(PHONE);
    test.settle();
    let ids = slide_ids(&test);

    let strip = test.rect_of("presentation.filmstrip");
    let first = test.rect_of(&format!("presentation.slide.{}", ids[0]));
    let second = test.rect_of(&format!("presentation.slide.{}", ids[1]));
    assert!(
        strip.width() > strip.height() && second.left() > first.right(),
        "below the slide the filmstrip lays its slides out side by side, {strip:?} {first:?} {second:?}"
    );

    let grab = first.center();
    let past = test.point_of(&format!("presentation.slide.{}", ids[1])) + Vec2::new(10.0, 0.0);
    test.down(grab, PointerButton::Primary);
    test.next_frame();
    test.hover(grab + Vec2::new(6.0, 0.0));
    test.next_frame();
    test.hover(past);
    test.run();
    test.advance(Duration::from_millis(500));
    test.run();
    let carried = test.rect_of(&format!("presentation.slide.{}", ids[1]));
    assert!(
        carried.left() < second.left() - 1.0,
        "the slide it passes moves aside along the strip, {second:?} -> {carried:?}"
    );
    test.snapshot("a_slide_dragged_along_a_wide_filmstrip_passes_the_slide_beside_it");

    test.up(past, PointerButton::Primary);
    test.settle();
    assert_eq!(slide_ids(&test), [ids[1], ids[0], ids[2]]);
}
