use super::*;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn tapping_the_caret_handle_opens_a_menu_that_asks_the_host_to_paste() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="Hello" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(unstyled::text_input_text(harness.document(), input));
    let start = pos2(text.left() + 1.0, text.center().y);
    harness.touch(TouchPhase::Start, start);
    harness.touch(TouchPhase::End, start);
    harness.frame(Vec::new());

    let handles = unstyled::text_input_handles(harness.document(), input);
    let [handle] = handles[..] else {
        panic!("a caret placed by touch shows one handle, not {handles:?}");
    };
    harness.touch(TouchPhase::Start, handle);
    harness.touch(TouchPhase::End, handle);
    harness.frame(Vec::new());

    let paste = unstyled::text_input_menu_row(harness.document(), input, 0)
        .expect("tapping the caret handle opens its menu");
    let row = harness.center(paste);
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
    assert!(output.paste_requested);

    harness.frame(vec![Event::Text("Say ".to_owned())]);
    assert_eq!(
        styled::text_input_value(harness.document(), input),
        "Say Hello"
    );
}
