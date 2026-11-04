use super::*;
use crate::styled::*;

#[test]
fn ctrl_a_selects_everything_so_typing_replaces_the_value() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="hello" />
        }]
    });
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::A, Modifiers::CTRL);
    harness.type_text("bye");

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "bye"
    );
}
