use super::*;
use crate::reactive::view;
use crate::styled::{NumberDrag, NumberInput, number_input_field, number_input_text};

#[test]
fn dragging_a_number_input_sideways_changes_its_value() {
    let _root = crate::reactive::RootScope::new();
    let (changes, set_changes) = create_signal(Vec::new());
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <NumberInput
                value=4.0
                min=0.0
                max=100.0
                drag={NumberDrag::Linear { speed: 0.5 }}
                on_change={move |value| set_changes.update(|changes| changes.push(value))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let middle = harness.center(input);

    harness.drag(middle, middle + Vec2::new(20.0, 0.0));
    harness.frame(Vec::new());

    assert_eq!(
        changes.with_untracked(|changes| changes.last().copied()),
        Some(14.0)
    );
    let shown = number_input_text(harness.document(), input)
        .expect("a drag must not open the field for editing");
    assert_eq!(text_of(harness.document(), shown), "14");

    set_changes.update(|changes| changes.clear());
    harness.click(middle);
    harness.frame(Vec::new());

    assert!(
        changes.with_untracked(|changes| changes.is_empty()),
        "a click without a drag must not change the value"
    );
    let field = number_input_field(harness.document(), input)
        .expect("a click without a drag must open the field");
    assert!(unstyled::text_input_focused(harness.document(), field).get());
}
