use super::*;

#[test]
fn the_filter_menu_keeps_only_the_rows_of_the_values_ticked() {
    let mut tracker = tracker();
    let project = tracker.project;

    tracker.fixture.harness.click("database-view.filters.add");
    tracker
        .fixture
        .harness
        .click(format!("database-view.filters.field.{project}"));
    let filter = tracker
        .fixture
        .view_state()
        .filters
        .get_index(0)
        .unwrap()
        .id;
    tracker.fixture.harness.click(format!(
        "database-view.filter.{}.choice.0",
        filter.as_uuid()
    ));
    tracker
        .fixture
        .harness
        .snapshot("the_filter_menu_keeps_only_the_rows_of_the_values_ticked");

    assert!(
        tracker.fixture.harness.shown(&tracker.line(1)),
        "small protocols is Wayland"
    );
    assert!(
        !tracker.fixture.harness.shown(&tracker.line(2)),
        "the Beui row is filtered out"
    );
    assert!(
        !tracker.fixture.harness.shown(&tracker.line(3)),
        "a row with no project is too"
    );
}
