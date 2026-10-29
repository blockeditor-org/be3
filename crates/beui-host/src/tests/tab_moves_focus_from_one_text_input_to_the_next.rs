use super::*;
use crate::styled::*;

#[test]
fn tab_moves_focus_from_one_text_input_to_the_next() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! {
                <TextInput value=String::new() />
            },
            view! {
                <TextInput value=String::new() />
            },
        ]
    });
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("second");

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(first)
            .value(),
        ""
    );
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(second)
            .value(),
        "second"
    );
}
