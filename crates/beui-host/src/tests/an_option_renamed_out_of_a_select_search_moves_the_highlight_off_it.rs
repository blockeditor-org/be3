use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn an_option_renamed_out_of_a_select_search_moves_the_highlight_off_it() {
    let mut rename = None;
    let (document, [select]) = toolbar_of(|| {
        let label = create_signal("Mango".to_owned());
        rename = Some(label);
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
            <unstyled::ChoiceOption label={label} />
        };
        [view! {
            <Select options selected=None />
        }]
    });
    let rename = rename.expect("the options were built");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let trigger = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .trigger();
    harness.click(harness.center(trigger));
    harness.frame(Vec::new());
    harness.type_text("an");
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi>(select)
            .highlighted(),
        Some(2),
        "Mango is the second option the search shows"
    );

    with_installed(harness.document_mut(), |_| rename.set("Cherry".to_owned()));
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi>(select)
            .highlighted(),
        Some(1),
        "an option the search no longer shows gives the highlight to the first one it does"
    );
}
