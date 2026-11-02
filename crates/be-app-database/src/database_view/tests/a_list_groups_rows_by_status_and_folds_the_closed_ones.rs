use super::*;
use crate::database_view::app::list::group_test_key;
use crate::database_view::groups::GroupKey;

#[test]
fn a_list_groups_rows_by_status_and_folds_the_closed_ones() {
    let mut tracker = tracker();
    let shipped = tracker.line(6);
    let review = group_test_key(GroupKey::Option(tracker.review));
    let done = group_test_key(GroupKey::Option(tracker.done));

    assert!(tracker.fixture.harness.shown(&tracker.line(0)));
    assert!(
        !tracker.fixture.harness.shown(&shipped),
        "a closed group starts folded"
    );
    tracker.fixture.harness.record();

    tracker
        .fixture
        .harness
        .click(format!("database-view.group.{done}.toggle"));
    tracker.fixture.settle();
    assert!(tracker.fixture.harness.shown(&shipped));

    tracker
        .fixture
        .harness
        .click(format!("database-view.group.{review}.toggle"));
    tracker.fixture.settle();
    assert!(!tracker.fixture.harness.shown(&tracker.line(1)));
    tracker.fixture.harness.record();
    tracker
        .fixture
        .harness
        .snapshot("a_list_groups_rows_by_status_and_folds_the_closed_ones");
}
