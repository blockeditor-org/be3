use accesskit::Role;

use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::view;
use crate::styled::DateTimeField;
use crate::unstyled::DateTimeParts;

#[test]
fn picking_a_day_from_a_date_fields_calendar_closes_it_and_returns_the_focus() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let document = build(move || {
        view! {
            <DateTimeField
                value={Some(DateTime::new(Date::new(2026, 2, 27), Time::new(8, 45)))}
                today={Some(Date::new(2026, 1, 1))}
                parts=DateTimeParts::Date
                label="Due"
                on_change={move |value| sink.borrow_mut().push(value)}
            />
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
        *reported.borrow(),
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
