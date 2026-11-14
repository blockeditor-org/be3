use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use beui_text_editor_core::TextBuffer;
use std::sync::Arc;

const TEXT: &str = "first line\nsecond line\nthird line";

#[test]
fn ctrl_home_and_ctrl_end_go_to_the_ends_of_a_text_area_and_shift_extends() {
    let _root = RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(TEXT.as_bytes())) as Arc<dyn beui_text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let state = held.get_untracked().expect("the text area was built");
    let canvas = harness.rect(state.canvas().get());
    harness.click(pos2(canvas.min.x + 40.0, canvas.min.y + 12.0));
    harness.settle();
    let end = TEXT.len();
    let shift = |ctrl| Modifiers {
        ctrl,
        shift: true,
        ..Modifiers::NONE
    };

    harness.key(Key::End, Modifiers::CTRL);
    assert_eq!(state.caret_indices(), vec![end]);
    harness.key(Key::Home, Modifiers::CTRL);
    assert_eq!(state.caret_indices(), vec![0]);

    harness.key(Key::ArrowDown, Modifiers::NONE);
    let middle = state.caret_indices()[0];
    assert!(
        middle > 0 && middle < end,
        "the caret is on the second line"
    );
    harness.key(Key::End, shift(true));
    assert_eq!(state.selection_ranges(), vec![middle..end]);
    harness.key(Key::Home, shift(true));
    assert_eq!(state.selection_ranges(), vec![0..middle]);

    // Cmd+Down and Cmd+Up on Apple's systems, where Command arrives as `ctrl`.
    harness.key(Key::ArrowDown, Modifiers::CTRL);
    assert_eq!(state.caret_indices(), vec![end]);
    harness.key(Key::ArrowUp, shift(true));
    assert_eq!(state.selection_ranges(), vec![0..end]);
}
