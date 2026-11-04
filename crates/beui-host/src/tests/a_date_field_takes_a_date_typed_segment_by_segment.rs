use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{Container, DateTimeParts};

#[test]
fn a_date_field_takes_a_date_typed_segment_by_segment() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Container>|_size|
                <DateTimeField
                    value={Binding::new(None, move |value| reported.update(|reported| reported.push(value)))}
                    parts=DateTimeParts::Date
                    label="Due"
                />
            </Container>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("2026");
    harness.type_text("9");
    assert!(
        reported.with_untracked(|reported| reported.is_empty()),
        "a date is reported once it is whole"
    );
    harness.type_text("27");

    assert_eq!(
        reported.get_untracked(),
        [Some(DateTime::new(Date::new(2026, 9, 27), Time::MIDNIGHT))]
    );
}
