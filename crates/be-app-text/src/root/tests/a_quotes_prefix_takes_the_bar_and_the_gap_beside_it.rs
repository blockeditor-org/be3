use super::*;

/// A quote's `>` owns its bar's span and the space after it the gap between the bar and the
/// text: the caret stands left of the bar before the `>`, just right of it after, and at the
/// text after the space, and selecting the prefix fills the bar and the gap, the space's mark
/// drawn in the gap.
#[test]
fn a_quotes_prefix_takes_the_bar_and_the_gap_beside_it() {
    let mut editor = editor("> Quoted\n");
    editor.click("text.surface");
    editor.key(Key::Home, Modifiers::CTRL);
    editor.snapshot("a_quotes_prefix_takes_the_bar_and_the_gap_beside_it_before");

    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.snapshot("a_quotes_prefix_takes_the_bar_and_the_gap_beside_it_after_the_mark");

    editor.key(Key::ArrowRight, Modifiers::NONE);
    editor.snapshot("a_quotes_prefix_takes_the_bar_and_the_gap_beside_it_after_the_space");

    editor.key(Key::ArrowLeft, Modifiers::SHIFT);
    editor.key(Key::ArrowLeft, Modifiers::SHIFT);
    editor.snapshot("a_quotes_prefix_takes_the_bar_and_the_gap_beside_it_selected");
}
