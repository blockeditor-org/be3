use super::*;
use crate::input::ImeEvent;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn an_ime_composition_is_written_as_it_is_composed_and_holds_the_keys_meanwhile() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"ab")) as Arc<dyn text_editor_core::Document>;
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
            />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);

    harness.frame(vec![
        Input::Ime(ImeEvent::Enabled),
        Input::Ime(ImeEvent::SetComposingText("ni".to_owned())),
    ]);
    harness.key(Key::Backspace, Modifiers::NONE);
    assert_eq!(
        String::from_utf8_lossy(&state.bytes()),
        "abni",
        "a composition is written as it is composed, and the keys that edit it are the input method's"
    );

    harness.frame(vec![Input::Ime(ImeEvent::CommitText("你".to_owned()))]);
    assert_eq!(String::from_utf8_lossy(&state.bytes()), "ab你");

    harness.key(Key::Backspace, Modifiers::NONE);
    assert_eq!(
        String::from_utf8_lossy(&state.bytes()),
        "ab",
        "keys edit the text again once the composition is committed"
    );
}
