use super::*;
use crate::reactive::{Frame, view};
use crate::styled::TextArea;
use crate::unstyled::TextAreaState;
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn the_emoji_menu_opens_below_the_caret_on_a_line_it_opened_on_before() {
    let area = NodeRef::new();
    let placed = area.clone();
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"first line\nsecond line\n"))
            as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <Frame padding_horizontal=200.0 padding_vertical=150.0>
                <TextArea @node_ref=&placed state={state} />
            </Frame>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let area = harness
        .document()
        .node_rect(area.get())
        .expect("the text area is laid out");
    let first = pos2(area.right() - 10.0, area.top() + 10.0);
    let last = pos2(area.right() - 10.0, area.bottom() - 10.0);

    for at in [first, last, first] {
        harness.click(at);
        harness.type_text(" :");
        harness.frame(Vec::new());

        let row = harness
            .document()
            .find_test_id("text.emoji.0")
            .expect("a colon opens the emoji menu");
        let menu = harness
            .document()
            .node_rect(row)
            .expect("the emoji menu is laid out");
        assert!(
            menu.left() >= area.left() && menu.top() >= area.top(),
            "the menu {menu:?} opens by the caret in the text area {area:?}, not in the corner of the window"
        );
        harness.key(Key::Escape, Modifiers::NONE);
    }
}
