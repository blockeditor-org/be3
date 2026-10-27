use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn hovering_a_select_option_moves_the_keyboard_highlight() {
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
    with_installed(harness.document_mut(), |_| {
        crate::native::focus_within(trigger);
    });
    harness.frame(Vec::new());
    harness.key(Key::ArrowDown, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        Some("Apple")
    );

    let cherry = harness
        .document()
        .api::<unstyled::SelectApi<&str>>(select)
        .option_button(&"Cherry")
        .expect("cherry is shown");
    let cherry_pos = harness.center(cherry);
    harness.frame(vec![Input::PointerMoved(cherry_pos)]);

    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        Some("Cherry")
    );

    harness.key(Key::ArrowUp, Modifiers::NONE);
    harness.frame(Vec::new());

    assert_eq!(
        harness
            .document()
            .api::<unstyled::SelectApi<&str>>(select)
            .highlighted(),
        Some("Banana")
    );
}
