use block_editor_beui::beui::Modifiers;
use block_editor_beui::beui::datetime::{Date, SECONDS_PER_DAY};

use super::*;

#[test]
fn clicking_a_day_in_the_calendar_adds_a_row_on_it() {
    let mut fixture = editor(&[
        ("Name", DatabaseFieldType::String),
        ("Starts", DatabaseFieldType::Datetime),
    ]);
    let starts = fixture.fields[1];

    fixture.harness.click("database-view.display");
    fixture.settle();
    fixture.harness.click("database-view.kind.Calendar");
    fixture.settle();
    assert_eq!(fixture.view_state().kind, DatabaseViewKind::Calendar);
    assert_eq!(
        fixture.view_state().calendar_start,
        Some(starts),
        "the calendar picks the first date field"
    );
    fixture.harness.key(Key::Escape, Modifiers::NONE);
    fixture.settle();

    let day = Date::new(2026, 3, 4).days();
    fixture
        .harness
        .click(format!("database-view.calendar.day.{day}").as_str());
    fixture.settle();

    let database = fixture.database();
    assert_eq!(database.rows.len(), 1);
    let row = database.rows.get_index(0).unwrap();
    assert_eq!(
        row.value(starts),
        Some(&DatabaseValue::Datetime(day * SECONDS_PER_DAY + 9 * 3600))
    );
    assert!(fixture.harness.shown("database-view.detail.close"));
}
