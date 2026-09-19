use super::*;
use crate::reactive::view;
use crate::styled::{Body, Heading};

#[test]
fn arrow_keys_walk_the_simulated_screen_reader_through_the_document() {
    let (document, [_heading, _body]) = toolbar_of(|| {
        [
            view! {
                <Heading content="Settings" />
            },
            view! {
                <Body content="Pick a theme" />
            },
        ]
    });
    let mut harness = Harness::sized(document, TALL_VIEWPORT);
    harness.enable_screen_reader();
    harness.frame(Vec::new());

    assert_eq!(
        harness.reader_items(),
        ["Settings, text", "Pick a theme, text"]
    );
    assert_eq!(harness.reading().as_deref(), Some("Settings, text"));

    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(harness.reading().as_deref(), Some("Pick a theme, text"));

    harness.key(Key::ArrowRight, Modifiers::NONE);
    assert_eq!(harness.reading().as_deref(), Some("Pick a theme, text"));
    assert_eq!(
        harness.spoken().as_deref(),
        Some("End of the document. Pick a theme, text")
    );

    harness.key(Key::ArrowLeft, Modifiers::NONE);
    assert_eq!(harness.reading().as_deref(), Some("Settings, text"));
}
