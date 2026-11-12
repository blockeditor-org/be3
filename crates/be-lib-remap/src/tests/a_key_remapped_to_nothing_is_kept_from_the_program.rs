use super::*;
use crate::edit::{Editing, RuleEdit};

#[test]
fn a_key_remapped_to_nothing_is_kept_from_the_program() {
    let mut engine = engine(
        r#"
        [[group]]
        rules = [
          { on = "CapsLock", send = "" },
          { on = "Insert", tap = "", hold_ms = 20 },
          { on = "WheelUp", send = "" },
          { on = "F13", steps = [{ tap = "" }, { tap = "A" }] },
        ]
        "#,
    );
    assert_eq!(engine.input(press("CapsLock"), NOW), Vec::new());
    assert_eq!(engine.input(release("CapsLock"), NOW), Vec::new());
    assert_eq!(engine.input(press("Insert"), NOW), Vec::new());
    assert_eq!(engine.input(release("Insert"), NOW), Vec::new());
    assert_eq!(engine.input(Input::Wheel { x: 0, y: -120 }, NOW), Vec::new());
    assert!(
        engine.input(press("F13"), NOW).contains(&press("A")),
        "a step of nothing is left out"
    );

    let mut editing = Editing::parse("[[group]]\nname = \"Everywhere\"\nrules = []\n")
        .expect("the text is TOML");
    editing.add_rule(0);
    let text = editing.text();
    assert!(text.contains(r#"{ on = "", send = "" }"#), "a new rule is nothing to nothing: {text}");
    let groups = text::parse(&text).expect("an unfinished rule reads");
    assert!(groups[0].rules.is_empty(), "and does nothing until what is pressed is chosen");

    editing.edit_rule(0, 0, RuleEdit::Held("Ctrl".to_owned(), true));
    assert_eq!(editing.outline()[0].rules[0].on, "Ctrl+");
    editing.edit_rule(0, 0, RuleEdit::Trigger("CapsLock".to_owned()));
    let text = editing.text();
    assert!(text.contains(r#"on = "Ctrl+CapsLock""#), "{text}");
    let groups = text::parse(&text).expect("a rule sending nothing reads");
    assert_eq!(groups[0].rules[0].action, Action::NOTHING);

    editing.edit_rule(0, 0, RuleEdit::Key("Ctrl+".to_owned()));
    assert!(
        text::parse(&editing.text()).is_err(),
        "a modifier held with no key is no chord"
    );
}
