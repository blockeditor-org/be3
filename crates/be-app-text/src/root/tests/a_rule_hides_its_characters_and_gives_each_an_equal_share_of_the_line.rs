use super::*;

/// A rule's characters are not drawn, and each takes an equal share of the line, so the caret
/// steps across the rule a third of the way at a time; a rule on the last line is a rule too.
#[test]
fn a_rule_hides_its_characters_and_gives_each_an_equal_share_of_the_line() {
    let mut editor = editor("Above\n\n---");
    editor.click("text.surface");
    editor.key(Key::End, Modifiers::CTRL);
    editor.key(Key::Home, Modifiers::NONE);
    editor.key(Key::ArrowRight, Modifiers::NONE);

    editor.snapshot("a_rule_hides_its_characters_and_gives_each_an_equal_share_of_the_line");
}
