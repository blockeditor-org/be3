use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_select_opened_with_a_mouse_shows_the_keyboard_for_its_search() {
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
    harness.click(harness.center(trigger));
    let area = harness.frame(Vec::new()).window.ime;
    assert!(unstyled::select_open(harness.document(), select));
    assert!(
        area.as_ref().is_some_and(|area| area.keyboard.shown()),
        "a mouse suggests a hardware keyboard, so the search turns the input method on: {area:?}"
    );
}
