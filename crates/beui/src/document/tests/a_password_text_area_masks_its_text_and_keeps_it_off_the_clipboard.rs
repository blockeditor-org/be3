use super::*;
use crate::reactive::view;
use crate::styled::{TextArea, text_area_surface};
use crate::unstyled::{TextAreaState, text_area_shown};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_password_text_area_masks_its_text_and_keeps_it_off_the_clipboard() {
    let area = NodeRef::new();
    let slot = area.clone();
    let document = build(move || {
        let document =
            Arc::new(TextBuffer::new("sécret".as_bytes())) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <TextArea @node_ref=&slot state={state} password=true />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let surface = text_area_surface(harness.document(), area.get());

    assert_eq!(text_area_shown(harness.document(), surface), "******");

    harness.click(pos2(300.0, 16.0));
    harness.key(Key::A, Modifiers::CTRL);
    let copied = harness.frame(vec![key_event(Key::C, true, Modifiers::CTRL)]);
    let cut = harness.frame(vec![key_event(Key::X, true, Modifiers::CTRL)]);

    assert_eq!(copied.copied_text, None);
    assert_eq!(cut.copied_text, None);
    assert_eq!(text_area_shown(harness.document(), surface), "******");
}
