use super::editor;
use block_ui_test::prelude::*;

#[test]
fn lists_and_quotes_are_painted_with_bullets_and_bars() {
    let mut editor = editor(concat!(
        "- First item\n",
        "- A list item long enough that it wraps onto another line, which starts under its text rather than under its bullet, however narrow or wide the window it is drawn in happens to be today\n",
        "  * Nested by two spaces\n",
        "- Another item\n",
        "  continued on a line of its own\n",
        "    * Nested by four spaces, drawn the same\n",
        "- [ ] A task long enough to wrap as well, so its second line starts under the text beside the checkbox, however narrow or wide the window it is drawn in happens to be today\n",
        "8. Numbered from eight\n",
        "9. Nine\n",
        "10. Ten, its dot under the one above, and long enough to wrap, lining its next line up with the text after the number, however narrow or wide the window it is drawn in happens to be\n",
        "    continued under the text of ten, and long enough to wrap so its next line hangs there too, however narrow or wide the window\n",
        "\n",
        "> A quote that is not italic, and is long enough to wrap so that its second line stays beside the bar, however narrow or wide the window it is drawn in happens to be today\n",
        "> - A quoted list\n",
        ">   continued inside the quote\n",
        ">   - nested inside it\n",
        "> > Nested quote\n",
        "lazily continued without its markers\n",
    ));
    editor.run();

    editor.snapshot("lists_and_quotes_are_painted_with_bullets_and_bars");
}
