use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use beui::BackGesture;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn back_and_a_press_outside_close_the_emoji_menu_and_back_the_find_bar() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    let offered = |harness: &Harness| {
        harness
            .document()
            .find_test_id("text.completion.0")
            .is_some()
    };

    harness.click(pos2(300.0, 16.0));
    harness.type_text(":hea");
    harness.frame(Vec::new());
    assert!(offered(&harness));
    assert!(
        harness
            .frame(Vec::new())
            .main()
            .window
            .requests
            .handles_back
    );
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    harness.frame(Vec::new());
    assert!(!offered(&harness), "back closes the menu");

    harness.type_text(" :hea");
    harness.frame(Vec::new());
    assert!(offered(&harness));
    harness.click(pos2(VIEWPORT.x - 5.0, VIEWPORT.y - 5.0));
    harness.frame(Vec::new());
    assert!(!offered(&harness), "and so does a press outside it");

    harness.click(pos2(300.0, 16.0));
    harness.key(Key::F, Modifiers::CTRL);
    harness.frame(Vec::new());
    assert!(state.find_open().get_untracked());
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    harness.frame(Vec::new());
    assert!(
        !state.find_open().get_untracked(),
        "back closes the find bar"
    );
}
