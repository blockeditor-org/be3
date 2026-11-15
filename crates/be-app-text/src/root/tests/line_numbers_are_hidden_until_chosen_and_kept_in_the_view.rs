use super::*;

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
    assert_eq!(
        super::text(&editor),
        text,
        "the choice is not kept in the text"
    );
    // Lines 10 to 12 have two digits, but the gutter is as wide as it was for one.
    editor.snapshot("line_numbers_are_shown_in_a_gutter_of_a_fixed_width");

    let kept = editor.content::<EditorViewContent>(Some(view));
    let mut reopened = opened(block, view, &text, kept);
    reopened.pick_menu("text.line-numbers");
    reopened.run();
    assert_eq!(line_numbers(&reopened, view), Some(false));
}
