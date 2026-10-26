use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_text_area_without_find_leaves_ctrl_f_to_the_app() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let searched = create_signal(0_usize);
    let document = build_node(move || {
        Action::new("app.search", "Search the app", move || {
            searched.update(|count| *count += 1);
        })
        .shortcut(Chord::ctrl(Key::F))
        .register();
        let document =
            Arc::new(TextBuffer::new(b"one two one")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <TextArea state={state} find=false />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");

    harness.click(pos2(300.0, 16.0));
    harness.key(Key::F, Modifiers::CTRL);
    harness.frame(Vec::new());

    assert!(!state.find_open().get_untracked(), "no find bar opens");
    assert_eq!(
        searched.get_untracked(),
        1,
        "Ctrl+F reaches the app's own action"
    );
}
