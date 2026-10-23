use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{Container, DateTimeParts};

#[test]
fn arrows_at_the_ends_of_a_date_field_leave_its_value_alone() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Container>|_size|
                <DateTimeField
                    value={Binding::new(Some(DateTime::new(Date::new(2026, 9, 27), Time::MIDNIGHT)), move |value| reported.update(|reported| reported.push(value)))}
                    parts=DateTimeParts::Date
                />
            </Container>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowLeft, Modifiers::NONE);
    for _ in 0..4 {
        harness.key(Key::ArrowRight, Modifiers::NONE);
    }

    assert!(reported.with_untracked(|reported| reported.is_empty()));
}
