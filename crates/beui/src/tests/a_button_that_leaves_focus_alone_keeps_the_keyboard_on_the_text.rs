use super::*;
use crate::primitives::Text;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::Button;

#[test]
fn a_button_that_leaves_focus_alone_keeps_the_keyboard_on_the_text() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [input, button]) = toolbar_of(move || {
        [
            view! {
                <TextInput value="" />
            },
            view! {
                <Button press_focus=false on_click={move || clicks.update(|clicks| *clicks += 1)}>
                    <Text>"Bold"</Text>
                </Button>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(input));
    assert!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .ime
            .is_some()
    );

    harness.click(harness.center(button));
    assert_eq!(clicks.get_untracked(), 1);
    assert!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .ime
            .is_some(),
        "the text keeps the focus and the keyboard"
    );
}
