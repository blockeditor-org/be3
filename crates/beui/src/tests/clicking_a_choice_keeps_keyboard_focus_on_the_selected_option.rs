use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn clicking_a_choice_keeps_keyboard_focus_on_the_selected_option() {
    let (document, [group]) = toolbar_of(|| {
        [view! {
            <RadioGroup
                options={view! {
                    <unstyled::ChoiceOption value="One" label="One" />
                    <unstyled::ChoiceOption value="Two" label="Two" />
                    <unstyled::ChoiceOption value="Three" label="Three" />
                }}
                selected=Some("One")
            />
        }]
    });
    let options = document.children(group);
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    harness.click(harness.center(options[1]));
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(group)
            .selected(),
        Some("Two")
    );
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi<&str>>(group)
            .selected(),
        Some("Three")
    );
}
