use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{Container, DateTimeParts};

#[test]
fn arrow_keys_step_a_date_segment_and_keep_the_day_in_its_month() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(Vec::new());
    let document = build(move || {
        view! {
            <Container>|_size|
                <DateTimeField
                    value={Some(DateTime::new(Date::new(2026, 12, 31), Time::new(9, 15)))}
                    parts=DateTimeParts::Date
                    on_change={move |value: Option<DateTime>| {
                        reported.update(|reported| reported.push(value.map(|value| value.date)))
                    }}
                />
            </Container>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowRight, Modifiers::NONE);
    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);

    assert_eq!(
        reported.get_untracked(),
        [
            Some(Date::new(2026, 1, 31)),
            Some(Date::new(2026, 12, 31)),
            Some(Date::new(2026, 11, 30)),
        ]
    );
}
