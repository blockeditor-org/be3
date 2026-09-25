use super::*;
use crate::reactive::{create_signal, view};
use crate::styled::{NumberInput, number_input_field, number_input_text};

#[test]
fn escape_puts_back_the_number_the_field_opened_with() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [input]) = toolbar_of(move || {
        let (value, set_value) = create_signal(7.0);
        [view! {
            <NumberInput
                value={value}
                on_change={move |next| {
                    sink.borrow_mut().push(next);
                    set_value.set(next);
                }}
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
    assert_eq!(changes.borrow().last().copied(), Some(35.0));

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(changes.borrow().last().copied(), Some(7.0));
    let shown = number_input_text(harness.document(), input).expect("escape closes the field");
    assert_eq!(text_of(harness.document(), shown), "7");
}
