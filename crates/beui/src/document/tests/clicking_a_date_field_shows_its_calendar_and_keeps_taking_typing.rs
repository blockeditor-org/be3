use accesskit::Role;

use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::view;
use crate::styled::DateTimeField;
use crate::unstyled::DateTimeParts;

#[test]
fn clicking_a_date_field_shows_its_calendar_and_keeps_taking_typing() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let field = NodeRef::new();
    let named = field.clone();
    let document = build(move || {
        view! {
            <List spacing=0.0>
                <DateTimeField
                    @node_ref=&named
                    value={Some(DateTime::new(Date::new(2026, 9, 27), Time::MIDNIGHT))}
                    parts=DateTimeParts::Date
                    label="Due"
                    on_change={move |value: Option<DateTime>| {
                        sink.borrow_mut().push(value.map(|value| value.date))
                    }}
                />
            </List>
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let open = |harness: &Harness| {
        harness.accessible().iter().any(|node| {
            node.role() == Role::Dialog && node.label() == Some("Choose a date for Due")
        })
    };
    let showing = |harness: &Harness, month: &str| {
        harness
            .accessible()
            .iter()
            .any(|node| node.role() == Role::Grid && node.label() == Some(month))
    };

    let rect = harness.rect(field.get());
    harness.click(pos2(rect.left() + 20.0, rect.center().y));
    harness.frame(Vec::new());
    assert!(open(&harness), "clicking the field shows the calendar");
    assert!(showing(&harness, "September 2026"));

    harness.type_text("1982");
    harness.frame(Vec::new());
    assert!(showing(&harness, "September 1982"), "the calendar follows what is typed");
    assert_eq!(*reported.borrow(), [Some(Date::new(1982, 9, 27))]);

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(!open(&harness));
    harness.type_text("10");
    assert_eq!(
        *reported.borrow(),
        [Some(Date::new(1982, 9, 27)), Some(Date::new(1982, 10, 27))],
        "the focus went back to the segment that was being typed in"
    );

    let rect = harness.rect(field.get());
    harness.click(pos2(rect.right() - 60.0, rect.center().y));
    harness.frame(Vec::new());
    assert!(open(&harness), "clicking the space beside the segments shows the calendar too");
}
