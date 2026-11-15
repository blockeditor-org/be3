use super::editor;
use be_sdk_test::prelude::*;

#[test]
fn markdown_is_painted_with_its_styles() {
    let mut editor = editor("# Heading\n\nPlain **bold** and `code`.\n");
    editor.run();

    editor.snapshot("markdown_is_painted_with_its_styles");
}
