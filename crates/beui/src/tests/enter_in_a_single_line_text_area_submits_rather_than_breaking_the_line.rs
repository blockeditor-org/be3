use super::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn enter_in_a_single_line_text_area_submits_rather_than_breaking_the_line() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let submitted = create_signal(0);
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"one")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <TextArea
                colors={area_colors()}
                look={area_look()}
                font_size=AREA_FONT_SIZE
                padding=AREA_PADDING
                state={state}
                single_line=true
                on_submit={move || submitted.update(|submitted| *submitted += 1)}
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");

    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(vec![Input::Text(" two\nthree".to_owned())]);

    assert_eq!(submitted.get_untracked(), 1);
    assert_eq!(String::from_utf8_lossy(&state.bytes()), "one twothree");
}
