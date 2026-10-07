use super::*;
use crate::reactive::view;
use crate::styled::TextArea;
use crate::unstyled::TextAreaState;
use accesskit::{Role, Toggled};
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

const TEXT: &str = "- [ ] buy milk\nplain line";
const HALF_CHECKBOX: f32 = 9.0;

fn caret_at(harness: &Harness, state: &TextAreaState, byte: usize) -> Rect {
    let canvas = harness.rect(state.canvas().get());
    state
        .layout()
        .get_untracked()
        .caret_rect(byte)
        .expect("the byte was laid out")
        .translate(canvas.min.to_vec2())
}

fn text(state: &TextAreaState) -> String {
    String::from_utf8_lossy(&state.bytes()).into_owned()
}

fn checkbox_toggled(harness: &Harness) -> Vec<Option<Toggled>> {
    harness
        .accessible()
        .into_iter()
        .filter(|node| node.role() == Role::CheckBox)
        .map(|node| node.toggled())
        .collect()
}

#[test]
fn clicking_a_markdown_checkbox_in_a_text_area_toggles_it_without_moving_the_caret() {
    let held: Rc<RefCell<Option<TextAreaState>>> = Rc::new(RefCell::new(None));
    let sink = held.clone();
    let document = build(move || {
        let document =
            Arc::new(TextBuffer::new(TEXT.as_bytes())) as Arc<dyn text_editor_core::Document>;
        let state = TextAreaState::new(document);
        state.execute(EditorCommand::SetLanguage(TextLanguage::Markdown));
        sink.replace(Some(state.clone()));
        view! {
            <TextArea state={state} />
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    let state = held.borrow().clone().expect("the text area was built");
    let plain = TEXT.find("plain").expect("the text has a plain line") + 3;
    let marker = TEXT.find('[').expect("the text has a checkbox");

    let caret = caret_at(&harness, &state, plain);
    harness.click(pos2(caret.min.x + 0.25, caret.center().y));
    assert_eq!(state.caret_indices(), vec![plain]);
    let focused = harness.document().focused_node();
    assert!(focused.is_some(), "the text area took the focus");
    assert_eq!(checkbox_toggled(&harness), vec![Some(Toggled::False)]);

    let checkbox = caret_at(&harness, &state, marker);
    let checkbox = pos2(checkbox.min.x + HALF_CHECKBOX, checkbox.center().y);
    harness.click(checkbox);
    harness.frame(Vec::new());
    assert_eq!(text(&state), "- [x] buy milk\nplain line");
    assert_eq!(state.caret_indices(), vec![plain], "the caret stays put");
    assert_eq!(
        harness.document().focused_node(),
        focused,
        "the text area keeps the focus"
    );
    assert_eq!(checkbox_toggled(&harness), vec![Some(Toggled::True)]);

    harness.click(checkbox);
    harness.frame(Vec::new());
    assert_eq!(text(&state), TEXT);
    assert_eq!(state.caret_indices(), vec![plain]);
    assert_eq!(checkbox_toggled(&harness), vec![Some(Toggled::False)]);
}
