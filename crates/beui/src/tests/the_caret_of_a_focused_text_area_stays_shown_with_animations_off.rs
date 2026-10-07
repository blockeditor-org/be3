use super::*;
use crate::Motion;
use crate::reactive::{Frame, Interactive, view};
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn the_caret_of_a_focused_text_area_stays_shown_with_animations_off() {
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"one")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <List spacing=0.0>
                <TextArea state={state} single_line=true />
                <Interactive focusable=true>
                    <Frame width=20.0 height=20.0 />
                </Interactive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.context().set_motion(Motion::Instant);
    harness.frame(Vec::new());

    harness.key(Key::Tab, Modifiers::NONE);

    assert_eq!(
        harness.frame(Vec::new()).repaint_after,
        Duration::MAX,
        "a caret that does not blink asks for no more frames"
    );
}
