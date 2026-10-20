use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_clicked_number_input_selects_its_text_until_enter() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <NumberInput
                value={Binding::new(1234.0, move |next| changes.update(|changes| changes.push(next)))}
            />
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
        changes.with_untracked(|changes| changes.is_empty()),
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
