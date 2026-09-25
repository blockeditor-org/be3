use super::*;
use crate::reactive::{Focusable, Frame, view};
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn the_caret_of_a_focused_text_area_blinks_on_a_deadline() {
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"one")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <List spacing=0.0>
                <TextArea state={state} single_line=true />
                <Focusable>
                    <Frame width=20.0 height=20.0 />
                </Focusable>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);

    harness.key(Key::Tab, Modifiers::NONE);
    let focused = harness.frame(Vec::new());
    assert!(focused.repaint_after > Duration::ZERO);
    assert!(focused.repaint_after <= BLINK);

    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
}
