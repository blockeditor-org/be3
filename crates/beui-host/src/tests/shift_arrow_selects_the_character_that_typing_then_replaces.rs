use super::*;
use crate::styled::*;

#[test]
fn shift_arrow_selects_the_character_that_typing_then_replaces() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="cat" />
        }]
    });
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::ArrowLeft, Modifiers::SHIFT);
    harness.type_text("r");

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "car"
    );
}
