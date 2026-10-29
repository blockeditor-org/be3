use super::*;
use crate::reactive::*;
use crate::styled::*;
use crate::unstyled::{TextAreaApi, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_password_text_area_masks_its_text_and_keeps_it_off_the_clipboard() {
    let _root = crate::reactive::RootScope::new();
    let area = NodeRef::new();
    let document = build_node(move || {
        let document =
            Arc::new(TextBuffer::new("sécret".as_bytes())) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <TextArea @node_ref=&area state={state} password=true />
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let surface = harness
        .document()
        .api::<TextAreaSurfaceApi>(area.get())
        .surface();

    assert_eq!(
        harness.document().api::<TextAreaApi>(surface).shown(),
        "******"
    );

    harness.click(pos2(300.0, 16.0));
    harness.key(Key::A, Modifiers::CTRL);
    let copied = harness.frame(pressed_with(Key::C, Modifiers::CTRL));
    let cut = harness.frame(pressed_with(Key::X, Modifiers::CTRL));

    assert_eq!(copied.copied_text, None);
    assert_eq!(cut.copied_text, None);
    assert_eq!(
        harness.document().api::<TextAreaApi>(surface).shown(),
        "******"
    );
}
