use super::*;
use crate::primitives::Text;
use crate::styled::*;
use crate::unstyled::Button;

#[test]
fn a_focused_text_input_asks_for_the_keyboard() {
    let (document, [input, button]) = toolbar_of(|| {
        [
            view! {
                <TextInput value="" />
            },
            view! {
                <Button>
                    <Text>"Go"</Text>
                </Button>
            },
        ]
    });
    let mut harness = Harness::new(document);
    assert_eq!(
        harness.frame(Vec::new()).main().window.clone().requests.ime,
        None
    );

    harness.click(harness.center(input));
    let area = harness.frame(Vec::new()).main().window.clone().requests.ime;
    assert!(
        area.as_ref()
            .is_some_and(|area| area.rect.contains(harness.center(input))),
        "the focused text input asks for the keyboard over itself: {area:?}"
    );

    harness.click(harness.center(button));
    assert_eq!(
        harness.frame(Vec::new()).main().window.clone().requests.ime,
        None,
        "a focused button takes no text"
    );
}
