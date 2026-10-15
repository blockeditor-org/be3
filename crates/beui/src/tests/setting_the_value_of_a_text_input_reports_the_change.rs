use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn setting_the_value_of_a_text_input_reports_the_change() {
    let _root = crate::reactive::RootScope::new();
    let (reported, set_reported) = create_signal(String::new());
    let (value, set_value) = create_signal(String::new());
    let (document, [_input]) = toolbar_of(|| {
        [view! {
            <TextInput
                value
                on_change={move |value| {
                    set_reported.set(value);
                }}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_installed(harness.document_mut(), |_| {
        set_value.set("typed for you".to_string());
    });

    assert_eq!(reported.get_untracked().as_str(), "typed for you");
}
