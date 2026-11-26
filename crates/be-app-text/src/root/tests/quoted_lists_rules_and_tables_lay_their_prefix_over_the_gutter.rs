use super::*;

/// A quote's prefix takes its gutter whatever the quote holds: a lazily continued inner level
/// keeps its text beside the outer level's bar rather than a column further in, a quoted list
/// nests after the bars, a quoted rule shares only the room past them, and a quoted table's
/// grid starts after them; selecting everything fills each prefix's columns.
#[test]
fn quoted_lists_rules_and_tables_lay_their_prefix_over_the_gutter() {
    let mut editor = editor(concat!(
        "> > Nested one\n",
        "> lazy two\n",
        "\n",
        "> - A quoted item\n",
        ">   continued\n",
        ">   - nested inside it\n",
        "\n",
        "> Above\n",
        ">\n",
        "> ---\n",
        "\n",
        "> | a | b |\n",
        "> | - | - |\n",
        "> | c | d |\n",
    ));
    editor.snapshot("quoted_lists_rules_and_tables_lay_their_prefix_over_the_gutter");

    editor.click("text.surface");
    editor.key(Key::A, Modifiers::CTRL);
    editor.snapshot("quoted_lists_rules_and_tables_lay_their_prefix_over_the_gutter_selected");
}
