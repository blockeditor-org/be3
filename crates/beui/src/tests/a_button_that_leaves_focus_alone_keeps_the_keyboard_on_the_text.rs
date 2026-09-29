use super::*;
use crate::reactive::Text;
use crate::styled::TextInput;
use crate::unstyled::Button;

#[test]
fn a_button_that_leaves_focus_alone_keeps_the_keyboard_on_the_text() {
    let clicks = std::rc::Rc::new(std::cell::Cell::new(0));
    let counted = clicks.clone();
    let (document, [input, button]) = toolbar_of(move || {
        [
            view! {
                <TextInput value="" />
            },
            view! {
                <Button press_focus=false on_click={move || counted.set(counted.get() + 1)}>
                    <Text string="Bold" />
                </Button>
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(input));
    assert!(harness.frame(Vec::new()).ime.is_some());

    harness.click(harness.center(button));
    assert_eq!(clicks.get(), 1);
    assert!(
        harness.frame(Vec::new()).ime.is_some(),
        "the text keeps the focus and the keyboard"
    );
}
