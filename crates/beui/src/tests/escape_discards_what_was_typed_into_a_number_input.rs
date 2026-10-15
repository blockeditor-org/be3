use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn escape_discards_what_was_typed_into_a_number_input() {
    let _root = crate::reactive::RootScope::new();
    let (changes, set_changes) = create_signal(Vec::new());
    let (previews, set_previews) = create_signal(Vec::new());
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <NumberInput
                value=7.0
                on_change={move |value| set_changes.update(|changes| changes.push(value))}
                on_preview={move |value| set_previews.update(|previews| previews.push(value))}
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
    assert_eq!(
        previews.with_untracked(|previews| previews.last().copied()),
        Some(Some(35.0))
    );

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(
        changes.with_untracked(|changes| changes.is_empty()),
        "escape must not change the value"
    );
    assert_eq!(
        previews.with_untracked(|previews| previews.last().copied()),
        Some(None)
    );
    let shown = number_input_text(harness.document(), input).expect("escape closes the field");
    assert_eq!(text_of(harness.document(), shown), "7");
}
