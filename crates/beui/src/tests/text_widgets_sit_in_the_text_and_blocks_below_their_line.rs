use super::*;
use crate::reactive::{Frame, NodeRef, view};
use crate::unstyled::{TextArea, TextAreaState, TextWidget};
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer};

const TEXT: &str = "see [a] here\n[big]\nafter";
const BLOCK: Vec2 = Vec2::new(120.0, 60.0);

#[test]
fn text_widgets_sit_in_the_text_and_blocks_below_their_line() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let block = NodeRef::new();
    let popup = NodeRef::new();
    let document = build({
        let (block, popup) = (block.clone(), popup.clone());
        move || {
            let document =
                Arc::new(TextBuffer::new(TEXT.as_bytes())) as Arc<dyn text_editor_core::Document>;
            let state = TextAreaState::new(document);
            sink.replace(Some(state.clone()));
            let widgets = vec![
                TextWidget {
                    range: 4..7,
                    label: "Inline".to_owned(),
                    ..TextWidget::default()
                },
                TextWidget {
                    range: 13..18,
                    label: "Large".to_owned(),
                    block_size: Some(BLOCK),
                    ..TextWidget::default()
                },
            ];
            view! {
                <Frame width=600.0 height=400.0>
                    <TextArea
                        state={state}
                        widgets={widgets}
                        block={move |_: usize| view! { <Frame @node_ref=&block /> }}
                        selected_widget={move |_: usize| view! {
                            <Frame @node_ref=&popup width=40.0 height=20.0 />
                        }}
                    />
                </Frame>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");
    let canvas = harness.rect(state.canvas().get());
    let layout = state.layout().get_untracked();
    let caret = |byte: usize| {
        layout
            .caret_rect(byte)
            .expect("the byte was laid out")
            .translate(canvas.min.to_vec2())
    };

    let placed = harness.rect(block.get());
    assert_eq!(placed.size(), BLOCK, "a block widget gets the size it asked for");
    assert!(
        placed.min.y >= caret(13).max.y && placed.max.y <= caret(19).min.y,
        "the block sits between its line {:?} and the next {:?}: {placed:?}",
        caret(13),
        caret(19)
    );
    assert!(
        caret(7).min.x > caret(4).min.x,
        "an inline widget takes room in its line"
    );
    assert_eq!(caret(8).min.y, caret(4).min.y, "the text after it stays on the line");

    crate::reactive::with_reactive_scope(harness.document_mut(), || {
        let (anchor, focus) = {
            let core = state.core();
            (core.position(4), core.position(7))
        };
        state.execute(EditorCommand::SetSelection { anchor, focus });
    });
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    let shown = harness.rect(popup.get());
    assert!(
        shown.min.y >= caret(4).max.y - 4.0 && (shown.min.x - caret(4).min.x).abs() < 1.0,
        "a selected inline widget shows its popup below itself: {shown:?} under {:?}",
        caret(4)
    );
}
