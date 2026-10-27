use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn arrow_down_on_a_closed_select_trigger_opens_it_and_highlights_the_first_option() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption value="Apple" label="Apple" />
            <unstyled::ChoiceOption value="Banana" label="Banana" />
            <unstyled::ChoiceOption value="Cherry" label="Cherry" />
        };
        [view! {
            <Select options selected=None />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let trigger = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .trigger();
    with_installed(harness.document_mut(), |_| {
        crate::focus_within(trigger);
    });
    harness.frame(Vec::new());

    assert!(
        !harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .open()
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .open()
    );
    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        Some("Apple")
    );
}
