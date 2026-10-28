use super::*;

#[test]
fn a_new_row_takes_each_choices_default() {
    let mut tracker = tracker();
    let before = tracker.fixture.database().rows.len();

    tracker.fixture.harness.click("database-view.new");
    tracker.fixture.harness.type_text("Ship the redesign");

    let database = tracker.fixture.database();
    assert_eq!(database.rows.len(), before + 1);
    let row = database.rows.get_index(before).unwrap();
    assert_eq!(
        row.value(tracker.title),
        Some(&DatabaseValue::String("Ship the redesign".into()))
    );
    assert_eq!(
        row.value(tracker.status),
        Some(&DatabaseValue::Enum(tracker.backlog))
    );
}
