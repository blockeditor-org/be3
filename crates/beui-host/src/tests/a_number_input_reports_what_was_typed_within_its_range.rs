use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_number_input_reports_what_was_typed_within_its_range() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let previews = create_signal(Vec::new());
    let (document, [input]) = toolbar_of(move || {
        [view! {
            <NumberInput
                value={Binding::new(4.0, move |next| changes.update(|changes| changes.push(next)))}
                min=0.0
                max=10.0
                on_preview={move |value| previews.update(|previews| previews.push(value))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let shown = harness
        .document()
        .api::<NumberInputTextApi>(input)
        .text()
        .expect("the input starts closed");

    assert_eq!(text_of(harness.document(), shown), "4");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    let field = harness
        .document()
        .api::<NumberInputApi>(input)
        .field()
        .expect("enter opens the field");
    harness.type_text("42");
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(field)
            .shown(harness.document()),
        "42"
    );
    assert_eq!(
        previews.with_untracked(|previews| previews.last().copied()),
        Some(Some(10.0))
    );
    assert!(
        changes.with_untracked(|changes| changes.is_empty()),
        "typing only previews the value"
    );

    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(changes.get_untracked(), [10.0]);
    assert_eq!(
        previews.with_untracked(|previews| previews.last().copied()),
        Some(None)
    );
}
