use super::*;
use crate::styled::*;

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
        Input::Text("a".to_owned()),
        Input::PointerMoved(pos),
        Input::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
        },
        Input::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
        },
        Input::Text("b".to_owned()),
    ]);
    harness.frame(Vec::new());
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(first)
            .value(),
        "a"
    );
    assert_eq!(
        harness
            .document()
            .api::<styled::TextInputApi>(second)
            .value(),
        "b"
    );
}
