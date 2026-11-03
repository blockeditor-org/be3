use be_sdk_beui::beui::datetime::SECONDS_PER_DAY;
use be_type_database::database::{Database, DatabaseContent, DatabaseValue};
use be_type_database::database_schema::DatabaseSchemaContent;
use be_type_database::database_view::DatabaseViewContent;

use super::*;

#[test]
fn the_clock_lists_the_days_events_and_opens_the_full_calendar() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.click("desktop.clock");
    fixture.settle();
    let calendar = fixture
        .calendar()
        .expect("the clock opens the desktop's calendar");
    assert!(fixture.says("No events"), "today starts with nothing on");

    let view = fixture
        .test
        .content::<DatabaseViewContent>(Some(calendar))
        .root();
    let database = view.database.expect("the calendar shows a database");
    let starts = view.calendar_start.expect("the calendar has a start field");
    let schema = fixture
        .test
        .content::<DatabaseContent>(Some(database))
        .root()
        .schema
        .expect("the database has a schema");
    let name = fixture
        .test
        .content::<DatabaseSchemaContent>(Some(schema))
        .root()
        .title_field()
        .expect("events have a name");
    let today = i64::try_from(be_sdk_beui::wall_clock().as_secs()).unwrap() / SECONDS_PER_DAY
        * SECONDS_PER_DAY;
    for (hour, title) in [(14, "Dentist"), (9, "Standup")] {
        let (_, add) = Database::add_row([
            (starts, DatabaseValue::Datetime(today + hour * 3600)),
            (name, DatabaseValue::String(title.to_owned())),
        ]);
        fixture.test.edit::<DatabaseContent>(Some(database), &add);
    }
    let (_, tomorrow) = Database::add_row([
        (
            starts,
            DatabaseValue::Datetime(today + SECONDS_PER_DAY + 3600),
        ),
        (name, DatabaseValue::String("Tomorrow's talk".to_owned())),
    ]);
    fixture
        .test
        .edit::<DatabaseContent>(Some(database), &tomorrow);
    fixture.settle();
    assert!(
        fixture.says("09:00") && fixture.says("Standup") && fixture.says("Dentist"),
        "today's events are listed"
    );
    assert!(
        !fixture.says("Tomorrow's talk") && !fixture.says("No events"),
        "only today's are"
    );
    for chrome in ["rows", "Filter and display", "Search"] {
        assert!(
            !fixture.says(chrome),
            "the popup has no editor's {chrome:?}"
        );
    }
    let document = fixture.test.document();
    let order: Vec<f32> = ["Standup", "Dentist"]
        .iter()
        .map(|title| {
            text_within(&document, document.root().unwrap(), title)
                .and_then(|node| document.node_rect(node))
                .expect("the event is shown")
                .min
                .y
        })
        .collect();
    drop(document);
    assert!(order[0] < order[1], "events are in the order they start");
    fixture
        .test
        .snapshot("the_clock_shows_a_month_and_the_days_events");

    fixture.test.click("desktop.calendar.open");
    fixture.settle();
    let placed = fixture.placed_blocks();
    let [(block, _, _)] = placed[..] else {
        panic!("the full calendar opens, and only it: {placed:?}");
    };
    assert_eq!(
        block, calendar,
        "Open calendar opens the desktop's calendar"
    );
    assert!(
        fixture.test.test_ids().rect("desktop.calendar").is_none(),
        "the popup closes as the calendar opens"
    );
}
