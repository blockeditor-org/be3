use super::editor;

#[test]
fn lists_and_quotes_are_painted_with_bullets_and_bars() {
    let mut editor = editor(
        "- First item\n- Second item\n  * Nested item\n1. Numbered\n2. Another\n\n> A quote that is not italic\n> > Nested quote\n",
    );
    editor.run();

    editor.snapshot("lists_and_quotes_are_painted_with_bullets_and_bars");
}
