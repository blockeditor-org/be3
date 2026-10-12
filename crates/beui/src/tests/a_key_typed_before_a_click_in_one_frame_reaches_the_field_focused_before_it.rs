use super::*;
use crate::styled::TextInput;

#[test]
fn a_key_typed_before_a_click_in_one_frame_reaches_the_field_focused_before_it() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! {
                <TextInput value="" />
            },
            view! {
                <TextInput value="" />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(harness.center(first));

    let pos = harness.center(second);
    harness.frame(vec![
        Event::Text("a".to_owned()),
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
        Event::Text("b".to_owned()),
    ]);
    assert_eq!(styled::text_input_value(harness.document(), first), "a");
    assert_eq!(styled::text_input_value(harness.document(), second), "b");
}
