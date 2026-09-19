use super::*;
use crate::reactive::view;
use crate::styled::{NumberInput, number_input_field};

#[test]
fn a_number_input_reports_what_was_typed_within_its_range() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <NumberInput
                value=4.0
                min=0.0
                max=10.0
                on_change={move |value| sink.borrow_mut().push(value)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let field = number_input_field(harness.document(), input);
    let text = unstyled::text_input_text(harness.document(), field);

    assert_eq!(text_of(harness.document(), text), "4");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("2");
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), text), "42");
    assert_eq!(*changes.borrow(), [10.0]);
}
