use block_editor_beui::beui::Vec2;

use super::*;

const NARROW: Vec2 = Vec2::new(390.0, 700.0);

#[test]
fn the_calendar_popup_fits_a_narrow_screen() {
    let mut fixture = Fixture::new();
    fixture.test.set_size(NARROW);
    fixture.settle();

    fixture.test.click("desktop.clock");
    fixture.settle();
    let placed = fixture.placed_blocks();
    let [(_, _, rect)] = placed[..] else {
        panic!("the calendar is placed: {placed:?}");
    };
    assert!(
        rect.min.x >= 0.0 && rect.max.x <= NARROW.x && rect.min.y >= 0.0 && rect.max.y <= NARROW.y,
        "the calendar stays on a {NARROW:?} screen, at {rect:?}"
    );
    let document = fixture.test.document();
    let clock = document
        .find_test_id("desktop.clock")
        .and_then(|clock| document.node_rect(clock))
        .expect("the clock is laid out");
    assert!(
        rect.max.y <= clock.min.y,
        "the calendar leaves the clock that opened it uncovered, at {rect:?}"
    );
}
