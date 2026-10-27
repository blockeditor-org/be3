use super::*;
use block_editor_beui::beui::Vec2;

#[test]
fn a_phone_opens_a_row_over_the_list_and_back_returns_to_it() {
    let mut tracker = tracker();
    tracker.fixture.harness.set_size(Vec2::new(390.0, 800.0));
    tracker.fixture.settle();
    let line = tracker.line(4);
    tracker.fixture.harness.record();

    tracker.fixture.harness.click(&line);
    assert!(tracker.fixture.harness.shown("database-view.detail.back"));
    assert!(
        !tracker.fixture.harness.shown(&line),
        "the row covers the list"
    );
    assert_eq!(
        tracker.fixture.harness.label("database-view.detail.id"),
        tracker.fixture.database().short_id(tracker.row(4))
    );
    tracker.fixture.harness.record();

    tracker.fixture.harness.click("database-view.detail.back");
    assert!(tracker.fixture.harness.shown(&line));
    tracker
        .fixture
        .harness
        .snapshot("a_phone_opens_a_row_over_the_list_and_back_returns_to_it");
}
