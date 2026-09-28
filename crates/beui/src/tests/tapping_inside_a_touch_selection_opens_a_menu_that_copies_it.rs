use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn tapping_inside_a_touch_selection_opens_a_menu_that_copies_it() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="Hello world" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(unstyled::text_input_text(harness.document(), input));
    let word = pos2(text.left() + 10.0, text.center().y);
    for _ in 0..2 {
        harness.touch(TouchPhase::Start, word);
        harness.touch(TouchPhase::End, word);
    }
    let inside = pos2(text.left() + 24.0, text.center().y);
    harness.touch(TouchPhase::Start, inside);
    harness.touch(TouchPhase::End, inside);
    harness.frame(Vec::new());

    let copy = unstyled::text_input_menu_row(harness.document(), input, 0)
        .expect("tapping the selection opens its menu");
    let row = harness.center(copy);
    harness.touch(TouchPhase::Start, row);
    let output = harness.frame(vec![Event::Touch {
        id: TouchId {
            device: 1,
            finger: 1,
        },
        phase: TouchPhase::End,
        pos: row,
        force: None,
    }]);

    assert_eq!(output.copied_text.as_deref().map(str::trim), Some("Hello"));
    assert!(unstyled::text_input_focused(harness.document(), input).get_untracked());
    assert_eq!(
        unstyled::text_input_selection(harness.document(), input).first(),
        Some(&(0..5))
    );
}
