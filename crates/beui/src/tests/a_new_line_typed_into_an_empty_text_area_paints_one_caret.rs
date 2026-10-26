use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::TextBuffer;

#[test]
fn a_new_line_typed_into_an_empty_text_area_paints_one_caret() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let document = Arc::new(TextBuffer::new(b"")) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        view! {
            <Frame width=500.0 height=300.0>
                <TextArea
                    colors={area_colors()}
                    look={area_look()}
                    font_size=AREA_FONT_SIZE
                    padding=AREA_PADDING
                    state={state}
                />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.click(pos2(250.0, 20.0));
    harness.type_text("first");
    harness.key(Key::Enter, Modifiers::NONE);
    harness.type_text("second");
    harness.frame(Vec::new());

    let document = harness.document();
    let root = document.root().expect("the document has a root");
    let carets: Vec<_> = document
        .texts_within(root)
        .into_iter()
        .flat_map(|text| {
            document
                .text_carets(text)
                .iter()
                .filter(|caret| caret.blink)
                .map(move |caret| (text, caret.at))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(carets.len(), 1, "one caret is painted, not {carets:?}");
}
