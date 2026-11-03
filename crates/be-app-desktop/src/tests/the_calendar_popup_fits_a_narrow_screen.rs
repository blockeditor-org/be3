use be_sdk_beui::beui::Vec2;

use super::*;

const NARROW: Vec2 = Vec2::new(360.0, 640.0);

#[test]
fn the_calendar_popup_fits_a_narrow_screen() {
    let mut fixture = Fixture::new();
    fixture.test.set_size(NARROW);
    fixture.settle();

    fixture.test.click("desktop.clock");
    fixture.settle();
    fixture.calendar().expect("the clock opens the calendar");
    let ids = fixture.test.test_ids();
    let rect = ids
        .rect("desktop.calendar")
        .expect("the calendar is laid out");
    let clock = ids.rect("desktop.clock").expect("the clock is laid out");
    drop(ids);
    assert!(
        rect.min.x >= 0.0 && rect.max.x <= NARROW.x && rect.min.y >= 0.0,
        "the calendar stays on a {NARROW:?} screen, at {rect:?}"
    );
    assert!(
        rect.max.y <= clock.min.y,
        "the calendar leaves the clock that opened it uncovered, at {rect:?}"
    );
    fixture
        .test
        .snapshot("the_calendar_popup_on_a_narrow_screen");
}
