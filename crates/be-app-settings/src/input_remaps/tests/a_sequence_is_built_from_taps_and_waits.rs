use super::*;

#[test]
fn a_sequence_is_built_from_taps_and_waits() {
    let mut editor = editor(RemapState::On, Some(CAPS_TO_ESCAPE));

    editor.click("input-remaps.group.0.rule.0");
    choose_does(&mut editor, "input-remaps.group.0.rule.0", 2);
    editor.click("input-remaps.group.0.rule.0.add-wait");
    editor.click("input-remaps.group.0.rule.0.add-tap");
    assert!(
        editor.shown("input-remaps.picker"),
        "a new tap asks for its key"
    );
    all_keys(&mut editor);
    editor.click("input-remaps.picker.key.x");

    let text = stored(&editor);
    let groups = be_lib_remap::text::parse(&text).expect("the sequence reads");
    let be_lib_remap::Action::Steps { steps, .. } = &groups[0].rules[0].action else {
        panic!("a sequence: {text}");
    };
    assert_eq!(steps.len(), 3, "{text}");
    assert!(
        text.contains(r#"steps = [{ tap = "Escape" }, { wait_ms = 10 }, { tap = "X" }]"#),
        "{text}"
    );

    editor.click("input-remaps.group.0.rule.0.step.0.remove");
    assert!(
        stored(&editor).contains(r#"steps = [{ wait_ms = 10 }, { tap = "X" }]"#),
        "{}",
        stored(&editor)
    );
}
