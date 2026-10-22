use super::*;
use crate::datetime::{DateTime, Time};
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{Container, DateTimeParts};

#[test]
fn a_time_field_picks_from_a_grid_of_times() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(Vec::new());
    let document = build_node(move || {
        view! {
            <Container>|_size|
                <DateTimeField
                    value={Binding::new(Some(DateTime::new(crate::datetime::Date::EPOCH, Time::new(10, 0))), move |value: Option<DateTime>| {
                        reported.update(|reported| reported.push(value.map(|value| value.time)))
                    })}
                    parts=DateTimeParts::Time
                />
            </Container>
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

    assert_eq!(reported.get_untracked(), [Some(Time::new(11, 15))]);
}
