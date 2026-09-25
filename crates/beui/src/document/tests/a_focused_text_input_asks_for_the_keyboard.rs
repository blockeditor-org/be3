use super::*;
use crate::reactive::{Button, Text};
use crate::styled::TextInput;

#[test]
fn a_focused_text_input_asks_for_the_keyboard() {
    let (document, [input, button]) = toolbar_of(|| {
        [
            view! {
                <TextInput value="" />
            },
            view! {
                <Button>
                    <Text string="Go" />
                </Button>
            },
        ]
    });
    let mut harness = Harness::new(document);
    assert_eq!(harness.frame(Vec::new()).ime, None);

    harness.click(harness.center(input));
    let area = harness.frame(Vec::new()).ime;
    assert!(
        area.is_some_and(|area| area.rect.contains(harness.center(input))),
        "the focused text input asks for the keyboard over itself: {area:?}"
    );

    harness.click(harness.center(button));
    assert_eq!(
        harness.frame(Vec::new()).ime,
        None,
        "a focused button takes no text"
    );
}
