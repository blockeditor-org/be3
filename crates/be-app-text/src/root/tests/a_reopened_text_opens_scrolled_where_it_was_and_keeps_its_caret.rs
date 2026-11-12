use std::time::Duration;

use be_sdk_beui::be_block::{EditorView, EditorViewContent};
use be_sdk_beui::{EditorHost, OwnedEditor};
use be_sdk_test::BeuiTest;
use be_type_text::{TextBlock, TextContent};
use beui::Vec2;
use uuid::Uuid;

use super::{PEER, shown_cursor};
use crate::root::TextRoot;
use crate::root::position::{POSITION, Position};

fn opened(
    block: Uuid,
    view: Uuid,
    content: TextContent,
    saved: EditorViewContent,
) -> BeuiTest<TextRoot> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_view_block(Some(view));
    let mut editor = BeuiTest::new(OwnedEditor::new(host, block));
    editor.hold(None, content);
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

/// The line shown whole at the top of the text, and how far below the text's top it is.
fn top_line(editor: &BeuiTest<TextRoot>) -> (String, f32) {
    let document = editor.document();
    let surface = document
        .find_test_id("text.surface")
        .expect("the text is shown");
    let top = document.node_rect(surface).expect("laid out").min.y;
    let mut lines = Vec::new();
    let mut pending = vec![surface];
    while let Some(node) = pending.pop() {
        let detail = document.node_detail(node).unwrap_or_default();
        let shown = detail.trim_matches('"');
        if document.node_kind(node) == "text"
            && shown.starts_with("Line ")
            && let Some(rect) = document.node_rect(node)
            && rect.min.y >= top - 0.5
        {
            lines.push((shown.to_owned(), rect.min.y - top));
        }
        pending.extend(document.children(node));
    }
    lines
        .into_iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .expect("the text shows its lines")
}

/// Where the caret is, as the editor shows it to its peers: in a text loaded afresh, a
/// position's offset is its byte.
fn caret(editor: &mut BeuiTest<TextRoot>) -> usize {
    let cursor = shown_cursor(editor)
        .flatten()
        .expect("the editor shows its caret to its peers");
    assert_eq!(cursor["anchor"], cursor["focus"], "{cursor}");
    assert_eq!(cursor["focus"]["client"], 0, "{cursor}");
    cursor["focus"]["offset"].as_u64().expect("an offset") as usize
}

#[test]
fn a_reopened_text_opens_scrolled_where_it_was_and_keeps_its_caret() {
    let text_bytes = (1..=120)
        .map(|line| format!("Line {line}\n"))
        .collect::<String>();
    let (block, view) = (Uuid::new_v4(), Uuid::new_v4());
    let mut first = opened(
        block,
        view,
        TextBlock::of(&text_bytes),
        EditorView::document(Some(block)),
    );
    first.scroll("text.surface", Vec2::new(0.0, 900.0));
    first.run();
    first.click("text.surface");
    first.run();
    first.type_text("Y");
    first.run();
    first.advance(Duration::from_secs(1));
    first.run();
    let left = top_line(&first);
    assert_ne!(left.0, "Line 1", "the view was scrolled down");
    let (kept, position) = saved(&first, view);
    assert!(
        position.is_some(),
        "where the text was left is saved once it holds still"
    );

    let left_text = TextBlock::text(&first.content::<TextContent>(None));
    let after_y = left_text.find('Y').expect("the Y was typed") + 1;

    let mut reopened = opened(
        block,
        view,
        first.content::<TextContent>(None),
        kept.clone(),
    );
    let shown = top_line(&reopened);
    assert_eq!(
        shown.0, left.0,
        "the reopened text shows the same line at the top"
    );
    assert!((shown.1 - left.1).abs() < 1.0, "{shown:?} against {left:?}");
    assert_eq!(
        caret(&mut reopened),
        after_y,
        "the caret came back where it was left, after the Y typed there"
    );
    reopened.snapshot("a_reopened_text_opens_scrolled_where_it_was");

    // While it is closed, a peer adds a line at the top, with characters of more than one byte.
    let added = "Añadido € ✓\n";
    let mut content = first.content::<TextContent>(None);
    let edit = TextBlock::insert(&content, PEER, 0, added.as_bytes())
        .expect("the peer has somewhere to type");
    content.apply(&edit);
    let mut moved = opened(block, view, content, kept.clone());
    let shown = top_line(&moved);
    assert_eq!(
        shown.0, left.0,
        "the same line is at the top, though a line was added above it"
    );
    assert!((shown.1 - left.1).abs() < 1.0, "{shown:?} against {left:?}");
    assert_eq!(
        caret(&mut moved),
        after_y + added.len(),
        "the caret follows its text down"
    );

    // A peer puts a character of three bytes just before the Y, so the byte the caret was at is
    // now inside it: the caret is found by the text after it.
    let mut content = first.content::<TextContent>(None);
    let edit = TextBlock::insert(&content, PEER, after_y - 1, "€".as_bytes())
        .expect("the peer has somewhere to type");
    content.apply(&edit);
    let mut squeezed = opened(block, view, content, kept);
    assert_eq!(top_line(&squeezed).0, left.0, "the same line is at the top");
    assert_eq!(
        caret(&mut squeezed),
        after_y + "€".len(),
        "the caret stays after the Y, not inside the €"
    );
}
