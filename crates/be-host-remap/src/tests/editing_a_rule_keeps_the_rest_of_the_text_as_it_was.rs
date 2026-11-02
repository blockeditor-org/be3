use super::*;
use crate::edit::{Does, Editing, GroupField, RuleEdit};

const WRITTEN: &str = r#"# my remaps
[[group]]
name = "Celeste" # the climbing one
window = { exe = "Celeste.exe" }
rules = [
  { on = "MouseLeft", send = "J" }, # jump
  { on = "MouseRight", send = "K" },
]
"#;

#[test]
fn editing_a_rule_keeps_the_rest_of_the_text_as_it_was() {
    let mut editing = Editing::parse(WRITTEN).expect("the text is TOML");
    editing.edit_rule(0, 1, RuleEdit::Key("L".to_owned()));
    assert_eq!(
        editing.text(),
        WRITTEN.replace(r#"send = "K""#, r#"send = "L""#),
        "only the key changed, comments and layout stay"
    );

    editing.edit_rule(0, 0, RuleEdit::Does(Does::Tap));
    editing.edit_rule(0, 0, RuleEdit::HoldMs(Some(20)));
    editing.add_rule(0);
    editing.set_group(0, GroupField::Exe, "");
    editing.add_group("Everywhere");
    editing.add_rule(1);
    let text = editing.text();
    assert!(
        text.starts_with("# my remaps\n[[group]]\nname = \"Celeste\" # the climbing one\n"),
        "{text}"
    );
    assert!(!text.contains("window"), "{text}");
    let groups = text::parse(&text).expect("what the editor writes reads");
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].rules.len(), 3);
    assert_eq!(
        groups[0].rules[0].action,
        Action::Steps {
            steps: vec![Step::Tap {
                chord: Chord {
                    modifiers: Vec::new(),
                    key: key("J"),
                },
                hold_ms: 20,
            }],
            blind: false,
        }
    );
    assert_eq!(groups[1].name, "Everywhere");
    assert_eq!(groups[1].rules.len(), 1);

    editing.remove_rule(0, 2);
    editing.remove_group(1);
    let groups = text::parse(&editing.text()).expect("it still reads");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].rules.len(), 2);
}
