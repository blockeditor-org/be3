use super::*;
use crate::datetime::{HourCycle, Time};
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{Container, DateTimeParts};

#[test]
fn a_twelve_hour_time_field_types_the_afternoon_with_a_p() {
    let _root = crate::reactive::RootScope::new();
    let (reported, set_reported) = create_signal(Vec::new());
    let document = build(move || {
        view! {
            <Container>|_size|
                <DateTimeField
                    value=None
                    parts=DateTimeParts::Time
                    hour_cycle=HourCycle::H12
                    on_change={move |value: Option<crate::datetime::DateTime>| {
                        set_reported.update(|reported| reported.push(value.map(|value| value.time)))
                    }}
                />
            </Container>
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
        reported.get_untracked(),
        [Some(Time::new(21, 30)), Some(Time::new(9, 30))]
    );
}
