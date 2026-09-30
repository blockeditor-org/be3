use super::*;
use crate::datetime::{DateTime, Time};
use crate::reactive::view;
use crate::styled::DateTimeField;
use crate::unstyled::DateTimeParts;

#[test]
fn a_time_field_picks_from_a_grid_of_times() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let document = build(move || {
        view! {
            <DateTimeField
                value={Some(DateTime::new(crate::datetime::Date::EPOCH, Time::new(10, 0)))}
                parts=DateTimeParts::Time
                on_change={move |value: Option<DateTime>| {
                    sink.borrow_mut().push(value.map(|value| value.time))
                }}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    for _ in 0..3 {
        harness.key(Key::Tab, Modifiers::NONE);
    }
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);

    assert_eq!(*reported.borrow(), [Some(Time::new(11, 15))]);
}
