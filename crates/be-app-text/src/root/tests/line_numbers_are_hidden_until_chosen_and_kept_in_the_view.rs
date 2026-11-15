use be_sdk_beui::be_block_content::{EditorView, EditorViewContent};
use beui::Vec2;

use super::*;
use crate::root::state::LINE_NUMBERS;

fn opened(block: Uuid, view: Uuid, text: &str, saved: EditorViewContent) -> BeuiTest<TextRoot> {
    let host = EditorHost::default();
    host.set_editable(true);
    host.set_view_block(Some(view));
    let mut editor = BeuiTest::new(OwnedEditor::new(host, block)).with_top_bar(false);
    editor.hold(None, TextBlock::of(text));
    editor.hold(Some(view), saved);
    editor.run();
    editor
}

fn line_numbers(editor: &BeuiTest<TextRoot>, view: Uuid) -> Option<bool> {
    editor
        .content::<EditorViewContent>(Some(view))
        .root()
        .state(LINE_NUMBERS)
        .and_then(|state| state.value::<bool>())
}

#[test]
fn line_numbers_are_hidden_until_chosen_and_kept_in_the_view() {
    let text = (1..=12)
        .map(|line| format!("Line {line}\n"))
        .collect::<String>();
    let (block, view) = (Uuid::new_v4(), Uuid::new_v4());
    let mut editor = opened(block, view, &text, EditorView::document(Some(block)));
    editor.snapshot("line_numbers_are_hidden_until_chosen");

    editor.pick_menu("text.line-numbers");
    editor.run();
    assert_eq!(line_numbers(&editor, view), Some(true));
    assert_eq!(super::text(&editor), text, "the choice is not kept in the text");
    // Lines 10 to 12 have two digits, but the gutter is as wide as it was for one.
    editor.snapshot("line_numbers_are_shown_in_a_gutter_of_a_fixed_width");

    let kept = editor.content::<EditorViewContent>(Some(view));
    let mut reopened = opened(block, view, &text, kept);
    reopened.pick_menu("text.line-numbers");
    reopened.run();
    assert_eq!(line_numbers(&reopened, view), Some(false));
}

#[test]
fn a_line_number_too_long_for_the_gutter_is_cut_to_its_last_digits() {
    let text = "x\n".repeat(10_011);
    let (block, view) = (Uuid::new_v4(), Uuid::new_v4());
    let mut editor = opened(block, view, &text, EditorView::document(Some(block)));
    editor.pick_menu("text.line-numbers");
    editor.run();
    editor.scroll("text.surface", Vec2::new(0.0, 1.0e7));
    editor.run();

    editor.snapshot("a_line_number_too_long_for_the_gutter_is_cut_to_its_last_digits");
}
