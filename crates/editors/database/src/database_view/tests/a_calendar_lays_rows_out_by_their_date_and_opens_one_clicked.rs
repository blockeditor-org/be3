use block_editor_beui::beui::datetime::{Date, SECONDS_PER_DAY};

use super::*;

#[test]
fn a_calendar_lays_rows_out_by_their_date_and_opens_one_clicked() {
    let mut fixture = editor(&[
        ("Name", DatabaseFieldType::String),
        ("Starts", DatabaseFieldType::Datetime),
        ("Ends", DatabaseFieldType::Datetime),
    ]);
    let (name, starts, ends) = (fixture.fields[0], fixture.fields[1], fixture.fields[2]);
    let day = Date::new(2026, 3, 4).days() * SECONDS_PER_DAY;
    fixture.set(0, name, DatabaseValue::String("Standup".into()));
    fixture.set(0, starts, DatabaseValue::Datetime(day + 9 * 3600));
    fixture.set(0, ends, DatabaseValue::Datetime(day + 10 * 3600));
    fixture.set(1, name, DatabaseValue::String("Review".into()));
    fixture.set(
        1,
        starts,
        DatabaseValue::Datetime(day + 2 * SECONDS_PER_DAY + 14 * 3600),
    );
    fixture.set(2, name, DatabaseValue::String("Someday".into()));
    fixture.edit_view(DatabaseView::set_calendar_start(Some(starts)));
    fixture.edit_view(DatabaseView::set_calendar_end(Some(ends)));
    fixture.edit_view(DatabaseView::set_kind(DatabaseViewKind::Calendar));
    fixture.settle();

    let standup = fixture.harness.rect_of("database-view.calendar.event.0");
    let review = fixture.harness.rect_of("database-view.calendar.event.1");
    assert!(standup.left() < review.left(), "each row sits on its day");
    assert!(
        !fixture.harness.shown("database-view.calendar.event.2"),
        "a row without a date is left out"
    );

    fixture.harness.click("database-view.calendar.event.1");
    fixture.settle();

    assert!(fixture.harness.shown("database-view.detail.close"));
    fixture
        .harness
        .snapshot("a_calendar_lays_rows_out_by_their_date_and_opens_one_clicked");
}
