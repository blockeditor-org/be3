use be_sdk_beui::beui::Vec2;

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const LANDSCAPE_PHONE: Vec2 = Vec2::new(844.0, 390.0);

fn in_view(tracker: &Tracker, line: &str) {
    let harness = &tracker.fixture.harness;
    let row = harness.rect_of(line);
    let stage = harness.rect_of("database-view.list");
    assert!(
        harness.shown(line) && stage.expand(0.5).contains_rect(row),
        "the opened row stays in view, {row:?} in {stage:?}"
    );
}

#[test]
fn a_phone_opens_a_row_below_the_list_and_keeps_it_in_view() {
    let mut tracker = tracker();
    tracker.fixture.harness.set_size(PHONE);
    tracker.fixture.settle();
    let line = tracker.line(5);

    tracker.fixture.harness.tap(&line);
    tracker.fixture.settle();
    assert!(tracker.fixture.harness.label("dock.tab.row").starts_with(&tracker.fixture.database().short_id(tracker.row(5))));
    let gathered = tracker.fixture.harness.rect_of("dock.compact");
    let detail = tracker.fixture.harness.rect_of("database-view.detail.title");
    assert!(
        gathered.contains_rect(detail),
        "the row opens in the pane gathered below the list, {detail:?} in {gathered:?}"
    );
    in_view(&tracker, &line);
    tracker
        .fixture
        .harness
        .snapshot("a_phone_opens_a_row_below_the_list");

    let splitter = tracker.fixture.harness.rect_of("dock.compact.splitter").center();
    tracker
        .fixture
        .harness
        .drag(splitter, splitter - Vec2::new(0.0, 250.0));
    tracker.fixture.settle();
    in_view(&tracker, &line);

    tracker.fixture.harness.set_size(LANDSCAPE_PHONE);
    tracker.fixture.settle();
    let gathered = tracker.fixture.harness.rect_of("dock.compact");
    let detail = tracker.fixture.harness.rect_of("database-view.detail.title");
    assert!(
        gathered.contains_rect(detail),
        "on a phone on its side the row opens right of the list, {detail:?} in {gathered:?}"
    );
    in_view(&tracker, &line);
    tracker
        .fixture
        .harness
        .snapshot("a_phone_on_its_side_opens_a_row_right_of_the_list");

    tracker.fixture.harness.tap("dock.tab.row.close");
    tracker.fixture.settle();
    assert!(
        !tracker.fixture.harness.shown("database-view.detail.title"),
        "closing the row's tab closes the row"
    );
}
