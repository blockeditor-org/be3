use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_key_pressed_in_a_held_select_search_shows_the_keyboard() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption value="Apple" label="Apple" />
            <unstyled::ChoiceOption value="Banana" label="Banana" />
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
    let at = harness.center(trigger);
    harness.touch(TouchPhase::Start, at);
    harness.touch(TouchPhase::End, at);
    let area = harness.frame(Vec::new()).main().window.clone().requests.ime;
    assert!(area.as_ref().is_some_and(|area| !area.keyboard.shown()));

    harness.key(Key::VolumeUp, Modifiers::NONE);
    let area = harness.frame(Vec::new()).main().window.clone().requests.ime;
    assert!(
        area.as_ref().is_some_and(|area| !area.keyboard.shown()),
        "a media key proves no keyboard with letters: {area:?}"
    );

    harness.key(Key::B, Modifiers::NONE);
    let area = harness.frame(Vec::new()).main().window.clone().requests.ime;
    assert!(
        area.as_ref().is_some_and(|area| area.keyboard.shown()),
        "a key typed into the search proves a hardware keyboard: {area:?}"
    );
}
