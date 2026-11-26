use super::*;

#[test]
fn a_line_number_too_long_for_the_gutter_is_cut_to_its_last_digits() {
    let text = "x\n".repeat(10_011);
    let (block, view) = (Uuid::new_v4(), Uuid::new_v4());
    let mut editor = opened(block, view, &text, EditorView::document(Some(block)));
    editor.pick_menu("text.line-numbers");
    editor.run();
    editor.scroll("text.surface", Vec2::new(0.0, 1.0e7));

    editor.snapshot("a_line_number_too_long_for_the_gutter_is_cut_to_its_last_digits");
}
