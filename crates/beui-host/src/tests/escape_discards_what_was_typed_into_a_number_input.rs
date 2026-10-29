use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn escape_discards_what_was_typed_into_a_number_input() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let previews = create_signal(Vec::new());
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <NumberInput
                value={Binding::new(7.0, move |next| changes.update(|changes| changes.push(next)))}
                on_preview={move |value| previews.update(|previews| previews.push(value))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let middle = harness.center(input);

    harness.click(middle);
    harness.frame(Vec::new());
    assert!(
        harness
            .document()
            .api::<NumberInputApi>(input)
            .field()
            .is_some()
    );
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
    let shown = harness
        .document()
        .api::<NumberInputTextApi>(input)
        .text()
        .expect("escape closes the field");
    assert_eq!(text_of(harness.document(), shown), "7");
}
