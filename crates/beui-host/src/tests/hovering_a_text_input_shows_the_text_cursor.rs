use super::*;
use crate::styled::*;
use beui::CursorIcon;

#[test]
fn hovering_a_text_input_shows_the_text_cursor() {
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <TextInput value="Hello" />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let text = harness.rect(
        harness
            .document()
            .api::<unstyled::TextInputApi>(input)
            .text(),
    );

    let output = harness.frame(vec![Input::PointerMoved(text.center())]);

    assert_eq!(output.main().window.requests.cursor, CursorIcon::Text);
}
