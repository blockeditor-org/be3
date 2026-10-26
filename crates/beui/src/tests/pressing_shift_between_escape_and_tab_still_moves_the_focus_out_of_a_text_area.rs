use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::*;
use std::sync::Arc;
use text_editor_core::TextBuffer;
use crate::primitives::List;

#[test]
fn pressing_shift_between_escape_and_tab_still_moves_the_focus_out_of_a_text_area() {
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"line")) as Arc<dyn text_editor_core::Document>;
        let state = crate::unstyled::TextAreaState::new(document);
        view! {
            <List spacing=8.0>
                <Button label="Before" variant=ButtonVariant::Secondary />
                <Frame height=100.0>
                    <TextArea state={state} />
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(300.0, 60.0));
    let editing = harness.document().focused_node();
    assert!(editing.is_some());

    harness.key(Key::Escape, Modifiers::NONE);
    harness.frame(vec![
        Input::Modifiers(Modifiers::SHIFT),
        key_event(Key::Shift, true),
    ]);
    harness.key(Key::Tab, Modifiers::SHIFT);
    assert_ne!(
        harness.document().focused_node(),
        editing,
        "the Shift of Shift+Tab is not a key the text area takes"
    );
}
