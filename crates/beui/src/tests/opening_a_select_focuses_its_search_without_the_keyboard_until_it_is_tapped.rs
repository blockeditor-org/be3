use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn opening_a_select_focuses_its_search_without_the_keyboard_until_it_is_tapped() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
        };
        [view! {
            <Select options selected=None />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let trigger = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .trigger();
    let at = harness.center(trigger);
    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    let area = harness.frame(Vec::new()).main().window.clone().requests.ime;
    assert!(harness.document().api::<unstyled::SelectApi>(select).open());
    assert!(
        area.as_ref().is_some_and(|area| !area.keyboard.shown()),
        "the search takes text but holds the on-screen keyboard back: {area:?}"
    );

    let search = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .search();
    harness.type_text("ban");
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(search)
            .value(),
        "ban",
        "a hardware keyboard types into the search before it is tapped"
    );

    let at = harness.center(search);
    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    let area = harness.frame(Vec::new()).main().window.clone().requests.ime;
    assert!(
        area.as_ref().is_some_and(|area| area.keyboard.shown()),
        "tapping the search asks for the on-screen keyboard: {area:?}"
    );
}
