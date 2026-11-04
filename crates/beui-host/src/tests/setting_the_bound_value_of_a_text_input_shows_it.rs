use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn setting_the_bound_value_of_a_text_input_shows_it() {
    let _root = crate::reactive::RootScope::new();
    let value = create_signal(String::new());
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value={value} />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_installed(harness.document_mut(), |_| {
        value.set("typed for you".to_string());
    });
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(input)
            .value(),
        "typed for you"
    );
}
