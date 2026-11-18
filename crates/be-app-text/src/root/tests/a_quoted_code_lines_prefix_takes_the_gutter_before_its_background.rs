use super::*;

/// A code block in a quote lays its lines' prefix over the quote's gutter as any quoted line
/// does, its background starting past the gutter: the caret stands left of the bar before the
/// `>`, just right of it after, and at the code after the space, and selecting the prefix
/// fills the bar and the gap beside it.
#[test]
fn a_quoted_code_lines_prefix_takes_the_gutter_before_its_background() {
    let mut editor = editor("> ```\n> let quoted = 1;\n> ```\n");
    editor.click("text.surface");
    editor.key(Key::Home, Modifiers::CTRL);
    editor.key(Key::ArrowDown, Modifiers::NONE);
    editor.key(Key::Home, Modifiers::NONE);
    editor.snapshot("a_quoted_code_lines_prefix_takes_the_gutter_before_its_background_before");

    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.snapshot("a_quoted_code_lines_prefix_takes_the_gutter_before_its_background_after_the_mark");

    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.snapshot("a_quoted_code_lines_prefix_takes_the_gutter_before_its_background_after_the_space");

    editor.key(Key::ArrowLeft, Modifiers::SHIFT);
    editor.key(Key::ArrowLeft, Modifiers::SHIFT);
    editor.snapshot("a_quoted_code_lines_prefix_takes_the_gutter_before_its_background_selected");
}
