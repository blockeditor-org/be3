use super::*;
use crate::styled::*;

#[test]
fn typing_into_a_focused_text_input_inserts_the_text() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value=String::new() />
        }]
    });
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("beui");

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "beui"
    );
}
