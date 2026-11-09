use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use beui_text_editor_core::TextBuffer;
use std::sync::Arc;

#[test]
fn a_text_areas_focus_request_is_taken_once_and_not_again_on_a_remount() {
    let _root = crate::reactive::RootScope::new();
    let area = NodeRef::new();
    let shown = create_signal(false);
    let document = Arc::new(TextBuffer::new(b"Notes")) as Arc<dyn beui_text_editor_core::Document>;
    let state = TextAreaState::new(document);
    let document = build_node(move || {
        view! {
            <Frame width=300.0 height=200.0>
                <List spacing=0.0>
                    <Show condition={shown}>
                        <TextArea
                            colors={area_colors()}
                            look={area_look()}
                            font_size=AREA_FONT_SIZE
                            padding=AREA_PADDING
                            @node_ref=&area
                            state={state}
                            line_numbers=false
                        />
                    </Show>
                </List>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    let focused_in_area = |harness: &Harness| harness.document().focus_is_within(area.get());

    state.focus();
    harness.frame(Vec::new());
    shown.set(true);
    harness.frame(Vec::new());
    assert!(
        focused_in_area(&harness),
        "a request made before the text area was shown takes effect once it is"
    );

    harness.document_mut().update_focus(None);
    shown.set(false);
    harness.frame(Vec::new());
    shown.set(true);
    harness.frame(Vec::new());
    assert!(
        !focused_in_area(&harness),
        "showing it again does not take the keyboard for the request it already took"
    );

    state.focus();
    harness.frame(Vec::new());
    assert!(focused_in_area(&harness), "a new request focuses it again");
}
