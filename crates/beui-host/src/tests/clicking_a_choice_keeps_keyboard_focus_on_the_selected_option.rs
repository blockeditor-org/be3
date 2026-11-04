use super::*;
use crate::styled::*;

#[test]
fn clicking_a_choice_keeps_keyboard_focus_on_the_selected_option() {
    let (document, [group]) = toolbar_of(|| {
        [view! {
            <RadioGroup
                options={view! {
                    <unstyled::ChoiceOption label="One" />
                    <unstyled::ChoiceOption label="Two" />
                    <unstyled::ChoiceOption label="Three" />
                }}
                selected=Some(0)
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
            .api::<styled::ChoiceApi>(group)
            .selected(),
        Some(1)
    );
    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::ChoiceApi>(group)
            .selected(),
        Some(2)
    );
}
