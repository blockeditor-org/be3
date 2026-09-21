use super::editor;

#[test]
fn markdown_is_painted_with_its_styles() {
    let (mut editor, _block) = editor("# Heading\n\nPlain **bold** and `code`.\n");
    editor.run();

    editor.snapshot("markdown_is_painted_with_its_styles");
}
