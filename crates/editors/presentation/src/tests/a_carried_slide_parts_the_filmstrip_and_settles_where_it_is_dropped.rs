use super::*;
use block_editor_beui::beui::{PointerButton, Vec2};
use std::time::Duration;

#[test]
fn a_carried_slide_parts_the_filmstrip_and_settles_where_it_is_dropped() {
    let (mut test, _editor) = editor(3);
    let ids = slide_ids(&test);
    test.run();

    let grab = test
        .rect_of(&format!("presentation.slide.{}", ids[0]))
        .center();
    let past = test
        .rect_of(&format!("presentation.slide.{}", ids[1]))
        .center()
        + Vec2::new(0.0, 10.0);
    test.down(grab, PointerButton::Primary);
    test.next_frame();
    test.hover(grab + Vec2::new(0.0, 6.0));
    test.next_frame();
    test.hover(past);
    test.run();
    test.record();
    for _ in 0..3 {
        test.advance(Duration::from_millis(50));
        test.record();
    }
    assert_eq!(
        slide_ids(&test),
        ids,
        "nothing moves until the slide is dropped"
    );

    test.up(past, PointerButton::Primary);
    test.run();
    test.record();
    test.settle();
    assert_eq!(slide_ids(&test), [ids[1], ids[0], ids[2]]);
    test.snapshot("a_carried_slide_parts_the_filmstrip_and_settles_where_it_is_dropped");
}
