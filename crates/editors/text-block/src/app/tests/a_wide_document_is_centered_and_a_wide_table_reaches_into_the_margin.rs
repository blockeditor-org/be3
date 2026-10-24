use beui::Vec2;

use super::editor;

#[test]
fn a_wide_document_is_centered_and_a_wide_table_reaches_into_the_margin() {
    let mut editor = editor(
        "# Centered\n\nA paragraph long enough to wrap at the column's width rather than at the edge of the window, so the margins on both sides stay empty.\n\n- a list item\n- another\n\n| Name | Description | Owner | Status | Due date | Notes |\n|------|-------------|-------|--------|----------|-------|\n| Planning | Write down what the quarter commits to | Alex | Done | March | Shared with the team |\n",
    )
    .with_size(Vec2::new(1400.0, 500.0));
    editor.run();

    editor.snapshot("a_wide_document_is_centered_and_a_wide_table_reaches_into_the_margin");
}
