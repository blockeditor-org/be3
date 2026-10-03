use super::*;
use crate::reactive::view;
use crate::styled::Select;

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

    let trigger = unstyled::select_trigger(harness.document(), select);
    let at = harness.center(trigger);
    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    let area = harness.frame(Vec::new()).ime;
    assert!(unstyled::select_open(harness.document(), select));
    assert!(
        area.as_ref().is_some_and(|area| !area.keyboard),
        "the search takes text but holds the on-screen keyboard back: {area:?}"
    );

    let search = unstyled::select_search(harness.document(), select);
    harness.type_text("ban");
    harness.frame(Vec::new());
    assert_eq!(
        unstyled::text_input_value(harness.document(), search),
        "ban",
        "a hardware keyboard types into the search before it is tapped"
    );

    let at = harness.center(search);
    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    let area = harness.frame(Vec::new()).ime;
    assert!(
        area.as_ref().is_some_and(|area| area.keyboard),
        "tapping the search asks for the on-screen keyboard: {area:?}"
    );
}
