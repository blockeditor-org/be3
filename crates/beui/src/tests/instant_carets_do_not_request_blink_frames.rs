use super::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use crate::{PreferenceLayer, PreferenceSource};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn instant_carets_do_not_request_blink_frames() {
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"one")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <List spacing=0.0>
                <TextArea
                    colors={area_colors()}
                    look={area_look()}
                    font_size=AREA_FONT_SIZE
                    padding=AREA_PADDING
                    state={state}
                    single_line=true
                />
                <Interactive focusable=true>
                    <Frame width=20.0 height=20.0 />
                </Interactive>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);

    harness.key(Key::Tab, Modifiers::NONE);
    harness.advance(BLINK);
    harness.frame(Vec::new());
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    harness.frame(Vec::new());
    assert_eq!(harness.frame(Vec::new()).repaint_after, Duration::MAX);
    harness.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Animated),
            ..PreferenceLayer::EMPTY
        },
    );
    assert!(harness.frame(Vec::new()).repaint_after <= BLINK);
}
