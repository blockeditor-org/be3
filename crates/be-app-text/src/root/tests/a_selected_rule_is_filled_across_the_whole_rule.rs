use super::*;

/// Selecting a rule fills it from its first caret stop to its last, the whole drawn rule, as
/// the carets at its ends stand.
#[test]
fn a_selected_rule_is_filled_across_the_whole_rule() {
    let mut editor = editor("Above\n\n---");
    editor.click("text.surface");
    editor.key(Key::End, Modifiers::CTRL);
    editor.key(Key::Home, Modifiers::SHIFT);

    editor.snapshot("a_selected_rule_is_filled_across_the_whole_rule");
}
