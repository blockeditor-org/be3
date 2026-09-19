use super::*;
use crate::reactive::view;
use crate::styled::{NumberDrag, NumberInput, number_input_field};

#[test]
fn dragging_a_number_input_sideways_changes_its_value() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let sink = changes.clone();
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <NumberInput
                value=4.0
                min=0.0
                max=100.0
                drag={NumberDrag::Linear { speed: 0.5 }}
                on_change={move |value| sink.borrow_mut().push(value)}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let field = number_input_field(harness.document(), input);
    let text = unstyled::text_input_text(harness.document(), field);
    let middle = harness.center(input);

    harness.drag(middle, middle + Vec2::new(20.0, 0.0));
    harness.frame(Vec::new());

    assert_eq!(changes.borrow().last().copied(), Some(14.0));
    assert_eq!(text_of(harness.document(), text), "14");
    assert!(
        !unstyled::text_input_focused(harness.document(), field).get(),
        "a drag must not leave the field being edited"
    );

    changes.borrow_mut().clear();
    harness.click(middle);
    harness.frame(Vec::new());

    assert!(
        unstyled::text_input_focused(harness.document(), field).get(),
        "a click without a drag must put the caret in the field"
    );
    assert!(
        changes.borrow().is_empty(),
        "a click without a drag must not change the value"
    );
}
