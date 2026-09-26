use super::*;
use crate::reactive::view;
use crate::styled::{NumberInput, number_input_field, number_input_text};

#[test]
fn a_clicked_number_input_selects_its_text_until_enter() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <NumberInput value=1234.0 on_change={move |value| sink.borrow_mut().push(value)} />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let middle = harness.center(input);

    harness.click(middle);
    harness.frame(Vec::new());
    let field = number_input_field(harness.document(), input).expect("a click opens the field");

    let opened = unstyled::text_input_selection(harness.document(), field);
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0], 0..4);

    let left = pos2(harness.rect(field).left() + 12.0, middle.y);
    harness.drag(left, left + Vec2::new(18.0, 0.0));
    harness.frame(Vec::new());

    assert!(
        changes.borrow().is_empty(),
        "dragging inside the field must not change the value"
    );
    let selection = unstyled::text_input_selection(harness.document(), field);
    assert!(
        selection.len() == 1 && !selection[0].is_empty() && selection[0] != (0..4),
        "dragging inside the field selects part of its text, not {selection:?}"
    );
    assert!(number_input_field(harness.document(), input).is_some());

    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(
        number_input_text(harness.document(), input).is_some(),
        "enter closes the field"
    );
}
