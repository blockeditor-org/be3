use accesskit::Role;

use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{Container, DateTimeParts};

#[test]
fn picking_a_day_from_a_date_fields_calendar_closes_it_and_returns_the_focus() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Container>|_size|
                <DateTimeField
                    value={Binding::new(Some(DateTime::new(Date::new(2026, 2, 27), Time::new(8, 45))), move |value| reported.update(|reported| reported.push(value)))}
                    today={Some(Date::new(2026, 1, 1))}
                    parts=DateTimeParts::Date
                    label="Due"
                />
            </Container>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let open = |harness: &Harness| {
        harness.accessible().iter().any(|node| {
            node.role() == Role::Dialog && node.label() == Some("Choose a date for Due")
        })
    };

    for _ in 0..4 {
        harness.key(Key::Tab, Modifiers::NONE);
    }
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        open(&harness),
        "the button beside the segments opens the calendar"
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        reported.get_untracked(),
        [Some(DateTime::new(Date::new(2026, 3, 6), Time::new(8, 45)))]
    );
    assert!(!open(&harness), "picking a day closes the calendar");
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(
        open(&harness),
        "the focus went back to the button that opened it"
    );
}
