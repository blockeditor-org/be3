use super::*;
use crate::reactive::view;
use crate::styled::{NumberInput, number_input_field, number_input_text};

#[test]
fn a_number_input_reports_what_was_typed_within_its_range() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let previews = Rc::new(RefCell::new(Vec::new()));
    let (change_sink, preview_sink) = (changes.clone(), previews.clone());
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <NumberInput
                value=4.0
                min=0.0
                max=10.0
                on_change={move |value| change_sink.borrow_mut().push(value)}
                on_preview={move |value| preview_sink.borrow_mut().push(value)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let shown = number_input_text(harness.document(), input).expect("the input starts closed");

    assert_eq!(text_of(harness.document(), shown), "4");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    let field = number_input_field(harness.document(), input).expect("enter opens the field");
    let text = unstyled::text_input_text(harness.document(), field);
    harness.type_text("42");
    harness.frame(Vec::new());

    assert_eq!(text_of(harness.document(), text), "42");
    assert_eq!(previews.borrow().last().copied(), Some(Some(10.0)));
    assert!(changes.borrow().is_empty(), "typing only previews the value");

    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(*changes.borrow(), [10.0]);
    assert_eq!(previews.borrow().last().copied(), Some(None));
}
