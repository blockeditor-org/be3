use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn the_find_bar_counts_matches_as_the_query_is_typed_and_steps_between_them() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"one two one two one"))
            as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        held.update(|held| *held = Some(state));
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.get_untracked().expect("the text area was built");
    let status = || state.find_status();

    harness.click(pos2(300.0, 16.0));
    harness.key(Key::F, Modifiers::CTRL);
    harness.frame(Vec::new());
    harness.type_text("one");
    harness.frame(Vec::new());

    assert_eq!(
        status().total,
        3,
        "every match of what was typed is counted"
    );

    harness.type_text(" t");
    harness.frame(Vec::new());
    let found = status();
    assert_eq!(found.total, 2, "the count follows the query as it grows");
    let first = found.current.expect("typing a query selects a match");

    let next = harness.find("text.find.next");
    let at = harness.center(next);
    harness.click(at);
    harness.frame(Vec::new());
    assert_eq!(
        status().current,
        Some((first + 1) % 2),
        "next moves to the following match"
    );
}
