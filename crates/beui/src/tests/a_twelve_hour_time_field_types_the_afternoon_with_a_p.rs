use super::*;
use crate::datetime::{HourCycle, Time};
use crate::reactive::view;
use crate::styled::DateTimeField;
use crate::unstyled::DateTimeParts;

#[test]
fn a_twelve_hour_time_field_types_the_afternoon_with_a_p() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let document = build(move || {
        view! {
            <DateTimeField
                value=None
                parts=DateTimeParts::Time
                hour_cycle=HourCycle::H12
                on_change={move |value: Option<crate::datetime::DateTime>| {
                    sink.borrow_mut().push(value.map(|value| value.time))
                }}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("9");
    harness.type_text("30");
    harness.type_text("p");
    harness.key(Key::ArrowUp, Modifiers::NONE);

    assert_eq!(
        *reported.borrow(),
        [Some(Time::new(21, 30)), Some(Time::new(9, 30))]
    );
}
