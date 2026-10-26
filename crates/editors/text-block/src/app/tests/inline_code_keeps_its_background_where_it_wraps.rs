use super::editor;

#[test]
fn inline_code_keeps_its_background_where_it_wraps() {
    let mut editor = editor(
        "A sentence long enough to reach the right edge of the block before its code begins, so `this inline code runs past the edge and wraps onto the next line` and the text goes on.\n",
    );
    editor.run();

    editor.snapshot("inline_code_keeps_its_background_where_it_wraps");
}
