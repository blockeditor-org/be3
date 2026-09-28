use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::reactive::{Text, view};
use crate::unstyled::{DateSegmentHandle, DateTimeField, date_time_field_text};

#[test]
fn a_half_typed_date_field_goes_back_to_its_value_when_the_focus_leaves() {
    let reported = Rc::new(RefCell::new(Vec::new()));
    let sink = reported.clone();
    let field = NodeRef::new();
    let named = field.clone();
    let document = build(move || {
        view! {
            <DateTimeField
                @node_ref=&named
                value={Some(DateTime::new(Date::new(2026, 9, 27), Time::new(10, 30)))}
                segment={|handle: DateSegmentHandle| view! {
                    <Text string={handle.text} />
                }}
                literal={|text: String| view! {
                    <Text string=text />
                }}
                on_change={move |value| sink.borrow_mut().push(value)}
            />
        }
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.frame(Vec::new());
    let field = field.get();

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Backspace, Modifiers::NONE);
    harness.type_text("20");
    assert_eq!(
        date_time_field_text(harness.document(), field),
        "20-09-27 10:30"
    );

    harness.document_mut().blur();
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert_eq!(
        date_time_field_text(harness.document(), field),
        "2026-09-27 10:30"
    );
    assert!(reported.borrow().is_empty());
}
