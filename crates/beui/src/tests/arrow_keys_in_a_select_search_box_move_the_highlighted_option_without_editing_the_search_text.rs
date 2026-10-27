use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn arrow_keys_in_a_select_search_box_move_the_highlighted_option_without_editing_the_search_text() {
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
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    let search = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .search();

    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        None
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        Some("Apple")
    );
    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(search)
            .value(),
        ""
    );

    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        Some("Banana")
    );
    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(search)
            .value(),
        ""
    );

    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        Some("Apple")
    );
}
