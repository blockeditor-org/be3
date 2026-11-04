use super::*;
use crate::edit::{Combo, Editing, RuleEdit};

const WRITTEN: &str = r#"[[group]]
name = "Everywhere"
rules = [
  { on = "F20+WheelUp", send = "VolumeUp" }, # louder
  { on = "Q", held = ["LeftShift"], tap = "Ctrl+W" },
]
"#;

#[test]
fn editing_a_trigger_keeps_what_is_held_with_it() {
    let mut editing = Editing::parse(WRITTEN).expect("the text is TOML");
    editing.edit_rule(0, 0, RuleEdit::Trigger("WheelDown".to_owned()));
    editing.edit_rule(0, 0, RuleEdit::Held("Ctrl".to_owned(), true));
    editing.edit_rule(0, 0, RuleEdit::Held("Shift".to_owned(), true));
    let outline = editing.outline();
    assert_eq!(outline[0].rules[0].on, "Shift+Ctrl+F20+WheelDown");

    editing.edit_rule(0, 0, RuleEdit::Held("F20".to_owned(), false));
    editing.edit_rule(0, 0, RuleEdit::NotHeld("Alt".to_owned(), true));
    editing.edit_rule(0, 1, RuleEdit::Held("Shift".to_owned(), false));
    editing.duplicate_rule(0, 0);
    let outline = editing.outline();
    let rules = &outline[0].rules;
    assert_eq!(rules.len(), 3);
    assert_eq!(rules[0].on, "Shift+Ctrl+WheelDown");
    assert_eq!(rules[0].not_held, ["Alt"]);
    assert_eq!(rules[1], rules[0], "the copy is just after it");
    assert!(rules[2].held.is_empty(), "a Shift key is Shift");

    let text = editing.text();
    assert_eq!(text.matches("# louder").count(), 1, "{text}");
    let groups = text::parse(&text).expect("what the editor writes reads");
    assert_eq!(groups[0].rules.len(), 3);

    let mut chord = Combo::parse("Ctrl+W");
    chord.hold("Shift");
    chord.release("LeftCtrl");
    assert_eq!(chord.text(), "Shift+W");
    assert_eq!(chord.modifiers(), [Modifier::Shift]);
}
