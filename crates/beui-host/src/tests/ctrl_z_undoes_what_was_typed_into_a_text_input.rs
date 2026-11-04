use super::*;
use crate::styled::*;

#[test]
fn ctrl_z_undoes_what_was_typed_into_a_text_input() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="note" />
        }]
    });
    let mut harness = Harness::new(document);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("s");
    harness.key(Key::Z, Modifiers::CTRL);

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "note"
    );
}
