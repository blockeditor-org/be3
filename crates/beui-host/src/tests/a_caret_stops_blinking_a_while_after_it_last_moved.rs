use super::*;
use crate::unstyled::{TextArea, TextAreaState};
use beui_text_editor_core::TextBuffer;
use std::sync::Arc;

/// `CARET_BLINKS_FOR`: ten blinks.
const BLINKS_FOR: Duration = Duration::from_millis(530 * 2 * 10);

#[test]
fn a_caret_stops_blinking_a_while_after_it_last_moved() {
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(b"one")) as Arc<dyn beui_text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <TextArea
                colors={area_colors()}
                look={area_look()}
                font_size=AREA_FONT_SIZE
                padding=AREA_PADDING
                state={state}
                single_line=true
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.key(Key::Tab, Modifiers::NONE);
    let focused = harness.frame(Vec::new());
    assert!(focused.repaint_after <= BLINK, "a focused caret blinks");
    let shown = focused.main().shapes();

    harness.advance(BLINK);
    let hidden = harness.frame(Vec::new());
    assert!(
        hidden.main().shapes() != shown,
        "half a blink later the caret is hidden"
    );

    harness.advance(BLINKS_FOR - BLINK);
    let still = harness.frame(Vec::new());
    assert_eq!(
        still.repaint_after,
        Duration::MAX,
        "after a while without moving, the caret asks for no more frames"
    );
    assert!(still.main().shapes() == shown, "and stays shown");

    harness.frame(vec![Input::Text("x".to_owned())]);
    assert!(
        harness.frame(Vec::new()).repaint_after <= BLINK,
        "typing starts it blinking again"
    );
}
