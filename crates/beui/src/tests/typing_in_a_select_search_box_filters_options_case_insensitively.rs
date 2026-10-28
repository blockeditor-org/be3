use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn typing_in_a_select_search_box_filters_options_case_insensitively() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption value="Apple" label="Apple" />
            <unstyled::ChoiceOption value="Banana" label="Banana" />
            <unstyled::ChoiceOption value="Grape" label="Grape" />
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
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());

    harness.type_text("BAN");
    harness.frame(Vec::new());

    let apple = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .option_button(&"Apple");
    let banana = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .option_button(&"Banana");
    let grape = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .option_button(&"Grape");
    assert!(apple.is_none());
    assert!(banana.is_some_and(|banana| harness.document().node_rect(banana).is_some()));
    assert!(grape.is_none());
    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        Some("Banana")
    );
}
