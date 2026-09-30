use super::*;
use crate::reactive::view;
use crate::styled::TextArea;
use crate::unstyled::TextAreaState;
use std::ops::Range;
use std::sync::Arc;
use text_editor_core::{EditorCommand, TextBuffer, TextLanguage};

const TEXT: &str = "first line here\nsecond line here\nthird line here";
const NEAR: f32 = 0.5;

fn at(harness: &Harness, state: &TextAreaState, byte: usize) -> Pos2 {
    let canvas = harness.rect(state.canvas().get());
    let rect = state
        .layout()
        .get_untracked()
        .caret_rect(byte)
        .expect("the byte was laid out")
        .translate(canvas.min.to_vec2());
    pos2(rect.min.x + 0.25, rect.center().y)
}

fn texts(harness: &Harness) -> Vec<NodeId> {
    let document = harness.document();
    let root = document.root().expect("the document has a root");
    document.texts_within(root)
}

fn painted_carets(harness: &Harness) -> Vec<Pos2> {
    let document = harness.document();
    texts(harness)
        .into_iter()
        .flat_map(|text| {
            document
                .text_carets(text)
                .iter()
                .filter(|caret| caret.blink)
                .filter_map(|caret| document.text_caret_rect(text, caret.at, caret.width))
                .map(|rect| pos2(rect.min.x, rect.center().y))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn painted_selection(harness: &Harness) -> Vec<(Pos2, Pos2)> {
    let document = harness.document();
    texts(harness)
        .into_iter()
        .flat_map(|text| {
            document
                .text_marks(text)
                .iter()
                .filter_map(|mark| {
                    let start = document.text_caret_rect(text, mark.range.start, 0.0)?;
                    let end = document.text_caret_rect(text, mark.range.end, 0.0)?;
                    Some((
                        pos2(start.min.x, start.center().y),
                        pos2(end.min.x, end.center().y),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

fn assert_near(painted: Pos2, expected: Pos2, what: &str) {
    assert!(
        (painted.x - expected.x).abs() <= NEAR && (painted.y - expected.y).abs() <= NEAR,
        "{what}: painted at {painted:?}, expected {expected:?}"
    );
}

fn assert_caret(harness: &Harness, state: &TextAreaState, byte: usize, what: &str) {
    assert_eq!(state.caret_indices(), vec![byte], "{what}");
    assert_eq!(state.selection_ranges(), vec![byte..byte], "{what}");
    let carets = painted_carets(harness);
    assert_eq!(
        carets.len(),
        1,
        "{what}: one caret is painted, not {carets:?}"
    );
    assert_near(
        carets[0] + Vec2::new(0.25, 0.0),
        at(harness, state, byte),
        what,
    );
    assert_eq!(painted_selection(harness), Vec::new(), "{what}");
}

fn assert_selection(harness: &Harness, state: &TextAreaState, range: Range<usize>, what: &str) {
    assert_eq!(state.selection_ranges(), vec![range.clone()], "{what}");
    let marks = painted_selection(harness);
    let (Some(first), Some(last)) = (marks.first(), marks.last()) else {
        panic!("{what}: no selection is painted");
    };
    let nudge = Vec2::new(0.25, 0.0);
    assert_near(first.0 + nudge, at(harness, state, range.start), what);
    assert_near(last.1 + nudge, at(harness, state, range.end), what);
}

#[test]
fn clicking_and_dragging_in_a_text_area_moves_the_caret_and_selects() {
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
    let second = TEXT.find("second").expect("the text has a second line");
    let third = TEXT.find("third").expect("the text has a third line");

    harness.click(at(&harness, &state, second + 3));
    assert_caret(&harness, &state, second + 3, "the first click");

    harness.click(at(&harness, &state, second + 9));
    assert_caret(&harness, &state, second + 9, "a click on the same line");

    harness.click(at(&harness, &state, third + 4));
    assert_caret(&harness, &state, third + 4, "a click on another line");

    harness.drag(at(&harness, &state, 1), at(&harness, &state, 8));
    assert_selection(&harness, &state, 1..8, "a drag within a line");

    harness.drag(
        at(&harness, &state, second + 2),
        at(&harness, &state, third + 5),
    );
    assert_selection(
        &harness,
        &state,
        second + 2..third + 5,
        "a drag across lines",
    );

    harness.click(at(&harness, &state, 5));
    assert_caret(&harness, &state, 5, "a click after a drag");
}
