use super::*;
use crate::styled::Text;
use crate::styled::*;

#[test]
fn alt_arrows_walk_the_simulated_screen_reader_through_the_document() {
    let (document, [_heading, _body]) = toolbar_of(|| {
        [
            view! {
                <Text style=TextStyle::HEADING>"Settings"</Text>
            },
            view! {
                <Text>"Pick a theme"</Text>
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

    harness.key(Key::ArrowRight, Modifiers::ALT);
    assert_eq!(harness.reading().as_deref(), Some("Pick a theme, text"));

    harness.key(Key::ArrowRight, Modifiers::ALT);
    assert_eq!(harness.reading().as_deref(), Some("Pick a theme, text"));
    assert_eq!(
        harness.spoken().as_deref(),
        Some("End of the document. Pick a theme, text")
    );

    harness.key(Key::ArrowLeft, Modifiers::ALT);
    assert_eq!(harness.reading().as_deref(), Some("Settings, text"));
}
