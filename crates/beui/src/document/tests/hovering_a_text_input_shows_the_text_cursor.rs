use super::*;
use crate::input::CursorIcon;
use crate::reactive::view;
use crate::styled::TextInput;

#[test]
fn hovering_a_text_input_shows_the_text_cursor() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="Hello" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(unstyled::text_input_text(harness.document(), input));

    let output = harness.frame(vec![Event::PointerMoved(text.center())]);

    assert_eq!(output.cursor_icon, CursorIcon::Text);
}
