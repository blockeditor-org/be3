use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn backspace_deletes_the_character_before_the_caret() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="beui" />
        }]
    });
    let mut harness = harness(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Backspace, Modifiers::NONE);

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "beu"
    );
}
