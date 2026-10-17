use crate::edit::{Does, Editing, RuleEdit, StepOutline};

#[test]
fn the_outline_shows_rules_as_written_even_before_they_read() {
    let mut editing = Editing::parse(
        "[[group]]\nname = \"Cairn\"\nrules = [{ on = \"Alt+Q\", not_held = [\"Shift\"], tap = \"Spcae\", hold_ms = 40, keep = true }]",
    )
    .expect("the text is TOML");
    let outline = editing.outline();
    let rule = &outline[0].rules[0];
    assert_eq!(rule.on, "Alt+Q");
    assert_eq!(rule.does, Does::Tap);
    assert_eq!(
        rule.key, "Spcae",
        "a name that does not read yet is shown as typed"
    );
    assert_eq!(rule.hold_ms, Some(40));
    assert_eq!(rule.not_held, "Shift");
    assert!(rule.keep);

    editing.edit_rule(0, 0, RuleEdit::Does(Does::Steps));
    editing.edit_rule(0, 0, RuleEdit::AddStep { wait: true });
    editing.edit_rule(0, 0, RuleEdit::AddStep { wait: false });
    editing.edit_rule(0, 0, RuleEdit::StepKey(2, "Q".to_owned()));
    editing.edit_rule(0, 0, RuleEdit::StepMs(1, Some(5)));
    editing.edit_rule(0, 0, RuleEdit::NotHeld(String::new()));
    let rule = editing.outline()[0].rules[0].clone();
    assert_eq!(
        rule.steps,
        vec![
            StepOutline::Tap {
                key: "Spcae".to_owned(),
                hold_ms: None,
            },
            StepOutline::Wait { ms: 5 },
            StepOutline::Tap {
                key: "Q".to_owned(),
                hold_ms: None,
            },
        ]
    );
    assert_eq!(rule.not_held, "");
}
