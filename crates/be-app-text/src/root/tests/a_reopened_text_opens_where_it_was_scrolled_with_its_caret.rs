use std::time::Duration;

use be_sdk_beui::be_block::{EditorView, EditorViewContent};
use be_sdk_beui::{EditorHost, OwnedEditor};
use be_sdk_test::BeuiTest;
use beui::Vec2;
use uuid::Uuid;

use crate::root::TextRoot;
use crate::root::position::{POSITION, Position};
use be_type_text::TextBlock;

fn opened(block: Uuid, view: Uuid, text: &str, saved: EditorViewContent) -> BeuiTest<TextRoot> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_view_block(Some(view));
    let mut editor = BeuiTest::new(OwnedEditor::new(host, block));
    editor.hold(None, TextBlock::of(text));
    editor.hold(Some(view), saved);
    editor.presence_visible(true);
    editor.run();
    editor
}

fn saved(editor: &BeuiTest<TextRoot>, view: Uuid) -> (EditorViewContent, Option<Position>) {
    let content = editor.content::<EditorViewContent>(Some(view));
    let position = content
        .root()
        .state(POSITION)
        .and_then(|state| state.value::<Position>());
    (content, position)
}

#[test]
fn a_reopened_text_opens_where_it_was_scrolled_with_its_caret() {
    let text = (1..=120)
        .map(|line| format!("Line {line}\n"))
        .collect::<String>();
    let (block, view) = (Uuid::new_v4(), Uuid::new_v4());
    let mut first = opened(block, view, &text, EditorView::document(Some(block)));
    first.scroll("text.surface", Vec2::new(0.0, 900.0));
    first.run();
    first.click("text.surface");
    first.run();
    first.advance(Duration::from_secs(1));
    first.run();
    let (kept, position) = saved(&first, view);
    let position = position.expect("where the text was left is saved once it holds still");
    assert!(
        position.line_start > 0,
        "the view was scrolled down: {position:?}"
    );
    assert!(
        position.focus > position.line_start,
        "the caret is below the top line"
    );

    let mut reopened = opened(block, view, &text, kept);
    reopened.advance(Duration::from_secs(1));
    reopened.run();
    let (_, again) = saved(&reopened, view);
    let again = again.expect("still saved");
    assert_eq!(
        (again.line_start, again.anchor, again.focus),
        (position.line_start, position.anchor, position.focus),
        "the reopened text shows the same line at the top, with the caret where it was"
    );
    assert!((again.within - position.within).abs() < 1.0);
    reopened.snapshot("a_reopened_text_opens_where_it_was_scrolled_with_its_caret");
}
