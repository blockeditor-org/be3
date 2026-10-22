use super::*;
use crate::datetime::{Date, DateTime, Time};
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{DateSegmentHandle, DateTimeField, DateTimeFieldApi};

#[test]
fn a_half_typed_date_field_goes_back_to_its_value_when_the_focus_leaves() {
    let _root = crate::reactive::RootScope::new();
    let reported = create_signal(Vec::new());
    let field = NodeRef::new();
    let document = build_node(move || {
        view! {
            <DateTimeField
                @node_ref=&field
                value={Binding::new(Some(DateTime::new(Date::new(2026, 9, 27), Time::new(10, 30))), move |value| reported.update(|reported| reported.push(value)))}
                segment={|handle: DateSegmentHandle| view! {
                    <Text>{handle.text}</Text>
                }}
                literal={|text: String| view! {
                    <Text>{text}</Text>
                }}
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
        harness.document().api::<DateTimeFieldApi>(field).text(),
        "20-09-27 10:30"
    );

    harness.document_mut().blur();
    harness.frame(Vec::new());
    harness.frame(Vec::new());

    assert_eq!(
        harness.document().api::<DateTimeFieldApi>(field).text(),
        "2026-09-27 10:30"
    );
    assert!(reported.with_untracked(|reported| reported.is_empty()));
}
