use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::view;
use crate::styled::DateTimeField;
use crate::unstyled::DateTimeParts;

#[test]
fn a_date_field_takes_a_date_typed_segment_by_segment() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let document = build(move || {
        view! {
            <DateTimeField
                value=None
                parts=DateTimeParts::Date
                label="Due"
                on_change={move |value| sink.borrow_mut().push(value)}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("2026");
    harness.type_text("9");
    assert!(reported.borrow().is_empty(), "a date is reported once it is whole");
    harness.type_text("27");

    assert_eq!(
        *reported.borrow(),
        [Some(DateTime::new(Date::new(2026, 9, 27), Time::MIDNIGHT))]
    );
}
