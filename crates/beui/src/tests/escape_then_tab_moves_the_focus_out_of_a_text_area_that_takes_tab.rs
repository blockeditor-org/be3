use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{TextAreaApi, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;
use crate::primitives::List;

#[test]
fn escape_then_tab_moves_the_focus_out_of_a_text_area_that_takes_tab() {
    let _root = crate::reactive::RootScope::new();
    let area = NodeRef::new();
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"line")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <List spacing=8.0>
                <Frame height=100.0>
                    <TextArea @node_ref=&area state={state} />
                </Frame>
                <Button label="After" variant=ButtonVariant::Secondary />
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let surface = harness
        .document()
        .api::<TextAreaSurfaceApi>(area.get())
        .surface();
    harness.click(pos2(300.0, 16.0));
    let editing = harness.document().focused_node();
    assert!(editing.is_some());

    harness.key(Key::Tab, Modifiers::NONE);
    assert_ne!(
        harness.document().api::<TextAreaApi>(surface).shown(),
        "line"
    );
    assert_eq!(harness.document().focused_node(), editing);
    let indented = harness.document().api::<TextAreaApi>(surface).shown();

    harness.key(Key::Escape, Modifiers::NONE);
    harness.key(Key::Tab, Modifiers::NONE);
    assert_eq!(
        harness.document().api::<TextAreaApi>(surface).shown(),
        indented
    );
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
