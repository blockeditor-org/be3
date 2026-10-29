use super::*;
use crate::styled::*;

#[test]
fn hovering_a_select_option_moves_the_keyboard_highlight() {
    let (document, [select]) = toolbar_of(|| {
        let options = view! {
            <unstyled::ChoiceOption label="Apple" />
            <unstyled::ChoiceOption label="Banana" />
            <unstyled::ChoiceOption label="Cherry" />
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
    with_installed(harness.document_mut(), |_| {
        crate::native::focus_within(trigger);
    });
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi>(select)
            .highlighted(),
        Some(0)
    );

    let cherry = harness
        .document()
        .api::<unstyled::SelectApi>(select)
        .option_button(2)
        .expect("cherry is shown");
    let cherry_pos = harness.center(cherry);
    harness.frame(vec![Input::PointerMoved(cherry_pos)]);

    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi>(select)
            .highlighted(),
        Some(2)
    );

    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi>(select)
            .highlighted(),
        Some(1)
    );
}
