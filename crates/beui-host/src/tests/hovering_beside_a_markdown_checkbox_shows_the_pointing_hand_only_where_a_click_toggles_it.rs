use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::TextAreaState;
use beui::CursorIcon;
use beui_text_editor_core::{EditorCommand, TextBuffer, TextLanguage};
use std::sync::Arc;

const TEXT: &str = "- [ ] buy milk";
const STEP: f32 = 2.0;

fn text(state: &TextAreaState) -> String {
    String::from_utf8_lossy(&state.bytes()).into_owned()
}

#[test]
fn hovering_beside_a_markdown_checkbox_shows_the_pointing_hand_only_where_a_click_toggles_it() {
    let _root = crate::reactive::RootScope::new();
    let held = create_signal::<Option<TextAreaState>>(None);
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new(TEXT.as_bytes())) as Arc<dyn beui_text_editor_core::Document>;
        let state = TextAreaState::new(document);
        state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
        held.update(|held| *held = Some(state));
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.settle();
    let state = held.get_untracked().expect("the text area was built");
    let checkbox = harness.rect(harness.find("text.checkbox.0"));
    let y = checkbox.center().y;
    let points = [
        ("just left of the box", pos2(checkbox.min.x - STEP, y), false),
        ("just inside its left edge", pos2(checkbox.min.x + STEP, y), true),
        ("just inside its right edge", pos2(checkbox.max.x - STEP, y), true),
        ("just right of the box", pos2(checkbox.max.x + STEP, y), false),
    ];
    for (place, at, inside) in points {
        let output = harness.frame(vec![Input::PointerMoved(at)]);
        let pointing = output.main().window.requests.cursor == CursorIcon::PointingHand;
        let before = text(&state);
        harness.click(at);
        harness.frame(Vec::new());
        let toggled = text(&state) != before;
        assert_eq!(
            (pointing, toggled),
            (inside, inside),
            "{place}: the pointing hand (left) and a toggling click (right) agree with the box",
        );
    }
}
