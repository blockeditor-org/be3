use super::*;
use crate::reactive::view;
use crate::styled::{NumberInput, number_input_field, number_input_text};

#[test]
fn escape_discards_what_was_typed_into_a_number_input() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let previews = Rc::new(RefCell::new(Vec::new()));
    let (change_sink, preview_sink) = (changes.clone(), previews.clone());
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <NumberInput
                value=7.0
                on_change={move |value| change_sink.borrow_mut().push(value)}
                on_preview={move |value| preview_sink.borrow_mut().push(value)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let middle = harness.center(input);

    harness.click(middle);
    harness.frame(Vec::new());
    assert!(number_input_field(harness.document(), input).is_some());
    harness.type_text("35");
    harness.frame(Vec::new());
    assert_eq!(previews.borrow().last().copied(), Some(Some(35.0)));

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(
        changes.borrow().is_empty(),
        "escape must not change the value"
    );
    assert_eq!(previews.borrow().last().copied(), Some(None));
    let shown = number_input_text(harness.document(), input).expect("escape closes the field");
    assert_eq!(text_of(harness.document(), shown), "7");
}
