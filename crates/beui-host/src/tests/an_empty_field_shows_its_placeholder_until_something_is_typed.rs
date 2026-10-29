use super::*;
use crate::styled::*;

#[test]
fn an_empty_field_shows_its_placeholder_until_something_is_typed() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value=String::new() placeholder="Search" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .shown(harness.document()),
        "Search"
    );

    harness.key(Key::Tab, Modifiers::NONE);
    harness.type_text("a");
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .shown(harness.document()),
        "a"
    );
}
