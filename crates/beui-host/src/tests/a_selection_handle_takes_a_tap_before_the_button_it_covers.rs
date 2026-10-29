use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_selection_handle_takes_a_tap_before_the_button_it_covers() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let (document, [input, _button]) = toolbar_of(move || {
        [
            view! {
                <TextInput value="Hello world" />
            },
            view! {
                <LabelledButton
                    label={"Below".to_owned()}
                    on_click={move || clicks.update(|clicks| *clicks += 1)}
                />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .text(),
    );
    let word = pos2(text.left() + 10.0, text.center().y);
    for _ in 0..2 {
        harness.touch(TouchPhase::Start, word);
        harness.touch(TouchPhase::End, word);
    }
    harness.frame(Vec::new());

    let handles = harness
        .document()
        .api::<unstyled::TextInputApi>(input)
        .handles(harness.document());
    let [_, end] = handles[..] else {
        panic!("a touch selection shows two handles, not {handles:?}");
    };
    let lower_edge_of_the_handle = pos2(end.x, end.y + 8.0);
    harness.touch(TouchPhase::Start, lower_edge_of_the_handle);
    harness.touch(TouchPhase::End, lower_edge_of_the_handle);
    harness.frame(Vec::new());

    assert_eq!(clicks.get_untracked(), 0);
    assert!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .focused()
            .get_untracked()
    );
    assert_eq!(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .selection()
            .first(),
        Some(&(0..5))
    );
}
