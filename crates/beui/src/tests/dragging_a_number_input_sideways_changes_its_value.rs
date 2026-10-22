use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn dragging_a_number_input_sideways_changes_its_value() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <NumberInput
                value={Binding::new(4.0, move |next| changes.update(|changes| changes.push(next)))}
                min=0.0
                max=100.0
                drag={NumberDrag::Linear { speed: 0.5 }}
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
    let shown = harness
        .document()
        .api::<NumberInputTextApi>(input)
        .text()
        .expect("a drag must not open the field for editing");
    assert_eq!(text_of(harness.document(), shown), "14");

    changes.update(|changes| changes.clear());
    harness.click(middle);
    harness.frame(Vec::new());

    assert!(
        changes.with_untracked(|changes| changes.is_empty()),
        "a click without a drag must not change the value"
    );
    let field = harness
        .document()
        .api::<NumberInputApi>(input)
        .field()
        .expect("a click without a drag must open the field");
    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(field)
            .focused()
            .get()
    );
}
