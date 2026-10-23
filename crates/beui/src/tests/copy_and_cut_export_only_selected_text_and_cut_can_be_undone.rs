use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn copy_and_cut_export_only_selected_text_and_cut_can_be_undone() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="Hello world" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    let output = harness.frame(pressed_with(Key::C, Modifiers::CTRL));
    assert_eq!(output.copied_text, None);
    harness.key(Key::Home, Modifiers::NONE);
    harness.key(
        Key::ArrowRight,
        Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        },
    );
    let output = harness.frame(pressed_with(Key::C, Modifiers::CTRL));
    assert_eq!(output.copied_text.as_deref().map(str::trim), Some("Hello"));
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "Hello world"
    );
    harness.key(Key::A, Modifiers::CTRL);
    let output = harness.frame(pressed_with(Key::X, Modifiers::CTRL));
    assert_eq!(output.copied_text.as_deref(), Some("Hello world"));
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        ""
    );
    harness.key(Key::Z, Modifiers::CTRL);
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "Hello world"
    );
    harness.key(Key::A, Modifiers::CTRL);
    harness.frame(vec![Input::Text("Pasted text".to_owned())]);
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "Pasted text"
    );
}
