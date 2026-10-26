use super::*;
use block_editor_beui::beui::Modifiers;

#[test]
fn searching_keeps_the_rows_whose_title_or_id_matches() {
    let mut tracker = tracker();

    tracker.fixture.harness.click("database-view.search");
    tracker.fixture.run();
    tracker.fixture.harness.type_text("SCALING");
    tracker.fixture.settle();
    assert!(tracker.fixture.harness.shown(&tracker.line(5)));
    assert!(!tracker.fixture.harness.shown(&tracker.line(4)));

    let code = tracker.fixture.database().short_id(tracker.row(4));
    for _ in "SCALING".chars() {
        tracker.fixture.harness.key(Key::Backspace, Modifiers::NONE);
    }
    tracker.fixture.harness.type_text(&code);
    tracker.fixture.settle();
    assert!(tracker.fixture.harness.shown(&tracker.line(4)));
    assert!(!tracker.fixture.harness.shown(&tracker.line(5)));
}
