use super::*;
use crate::reactive::view;
use crate::styled::{Heading, TextInput};

#[test]
fn keys_without_alt_reach_the_control_the_screen_reader_focused() {
    let (document, [_heading, input]) = toolbar_of(|| {
        [
            view! {
                <Heading content="Name" />
            },
            view! {
                <TextInput value=String::new() />
            },
        ]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.frame(Vec::new());

    harness.key(Key::ArrowRight, Modifiers::ALT);
    harness.frame(Vec::new());
    assert_eq!(harness.document().focused_node(), Some(input));

    harness.type_text("hi");
    harness.key(Key::ArrowLeft, Modifiers::NONE);
    harness.type_text("!");
    harness.frame(Vec::new());

    assert_eq!(styled::text_input_value(harness.document(), input), "h!i");
}
