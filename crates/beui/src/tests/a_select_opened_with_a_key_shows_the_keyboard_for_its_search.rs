use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_select_opened_with_a_key_shows_the_keyboard_for_its_search() {
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
    harness.frame(Vec::new());
    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(Vec::new());
    assert!(!unstyled::select_open(harness.document(), select));
    assert!(unstyled::button_state(harness.document(), trigger).get().focused);

    harness.key(Key::ArrowDown, Modifiers::NONE);
    let area = harness.frame(Vec::new()).window.ime;
    assert!(unstyled::select_open(harness.document(), select));
    assert!(
        area.as_ref().is_some_and(|area| area.keyboard.shown()),
        "a key proves a hardware keyboard, so the search turns the input method on: {area:?}"
    );
}
