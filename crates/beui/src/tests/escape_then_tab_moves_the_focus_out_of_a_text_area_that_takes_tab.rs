use super::*;
use crate::reactive::{Frame, List, view};
use crate::styled::{Button, ButtonVariant, TextArea, text_area_surface};
use crate::unstyled::{TextAreaState, text_area_shown};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn escape_then_tab_moves_the_focus_out_of_a_text_area_that_takes_tab() {
    let area = NodeRef::new();
    let slot = area.clone();
    let document = build(move || {
        let document = Arc::new(TextBuffer::new(b"line")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <List spacing=8.0>
                <Frame height=100.0>
                    <TextArea @node_ref=&slot state={state} />
                </Frame>
                <Button label="After" variant=ButtonVariant::Secondary />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let surface = text_area_surface(harness.document(), area.get());
    harness.click(pos2(300.0, 16.0));
    let editing = harness.document().focused_node();
    assert!(editing.is_some());

    harness.key(Key::Tab, Modifiers::NONE);
    assert_ne!(text_area_shown(harness.document(), surface), "line");
    assert_eq!(harness.document().focused_node(), editing);
    let indented = text_area_shown(harness.document(), surface);

    harness.key(Key::Escape, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(text_area_shown(harness.document(), surface), indented);
    assert_ne!(
        harness.document().focused_node(),
        editing,
        "Tab after Escape moves the focus on"
    );

    harness.key(Key::Tab, Modifiers::SHIFT);
    assert_eq!(harness.document().focused_node(), editing);
    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(
        harness.document().focused_node(),
        editing,
        "Tab indents again once the focus is back"
    );
}
