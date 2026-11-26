use super::*;

/// Each level of a nested quote lays its `>` and space over its own column, and a `>` with no
/// space after it owns its whole column, so selecting every line fills each prefix's columns.
#[test]
fn nested_and_tight_quotes_take_a_column_per_level() {
    let mut editor = editor("> > Nested\n\n>Tight\n");
    editor.click("text.surface");
    editor.key(Key::A, Modifiers::CTRL);

    editor.snapshot("nested_and_tight_quotes_take_a_column_per_level");
}
