use super::editor;

#[test]
fn lists_and_quotes_are_painted_with_bullets_and_bars() {
    let mut editor = editor(concat!(
        "- First item\n",
        "- A list item long enough that it wraps onto another line, which starts under its text rather than under its bullet, however narrow or wide the window it is drawn in happens to be today\n",
        "  * Nested item\n",
        "- [ ] A task long enough to wrap as well, so its second line starts under the text beside the checkbox, however narrow or wide the window it is drawn in happens to be today\n",
        "1. Numbered\n",
        "2. A numbered item that also runs long enough to wrap, lining its next line up with the text after the number, however narrow or wide the window it is drawn in happens to be today\n",
        "\n",
        "> A quote that is not italic, and is long enough to wrap so that its second line stays beside the bar, however narrow or wide the window it is drawn in happens to be today\n",
        "> > Nested quote\n",
    ));
    editor.run();

    editor.snapshot("lists_and_quotes_are_painted_with_bullets_and_bars");
}
