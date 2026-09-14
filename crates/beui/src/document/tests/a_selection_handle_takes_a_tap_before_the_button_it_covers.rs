use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn a_selection_handle_takes_a_tap_before_the_button_it_covers() {
    let clicks = Rc::new(Cell::new(0));
    let (document, [input, _button]) = toolbar_of({
        let clicks = clicks.clone();
        move || {
            [
                view! { <TextInput value="Hello world" /> },
                view! {
                    <LabelledButton
                        label={"Below".to_owned()}
                        on_click={move || clicks.set(clicks.get() + 1)}
                    />
                },
            ]
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(unstyled::text_input_text(harness.document(), input));
    let word = pos2(text.left() + 10.0, text.center().y);
    for _ in 0..2 {
        harness.touch(TouchPhase::Start, word);
        harness.touch(TouchPhase::End, word);
    }
    harness.frame(Vec::new());

    let handles = unstyled::text_input_handles(harness.document(), input);
    let [_, end] = handles[..] else {
        panic!("a touch selection shows two handles, not {handles:?}");
    };
    let lower_edge_of_the_handle = pos2(end.x, end.y + 8.0);
    harness.touch(TouchPhase::Start, lower_edge_of_the_handle);
    harness.touch(TouchPhase::End, lower_edge_of_the_handle);
    harness.frame(Vec::new());

    assert_eq!(clicks.get(), 0);
    assert!(unstyled::text_input_focused(harness.document(), input).get_untracked());
    assert_eq!(
        unstyled::text_input_selection(harness.document(), input).first(),
        Some(&(0..5))
    );
}
