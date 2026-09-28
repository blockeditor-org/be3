use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::view;
use crate::styled::DateTimeField;
use crate::unstyled::DateTimeParts;

#[test]
fn arrows_at_the_ends_of_a_date_field_leave_its_value_alone() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let document = build(move || {
        view! {
            <DateTimeField
                value={Some(DateTime::new(Date::new(2026, 9, 27), Time::MIDNIGHT))}
                parts=DateTimeParts::Date
                on_change={move |value| sink.borrow_mut().push(value)}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowLeft, Modifiers::NONE);
    for _ in 0..4 {
        harness.key(Key::ArrowRight, Modifiers::NONE);
    }

    assert!(reported.borrow().is_empty());
}
