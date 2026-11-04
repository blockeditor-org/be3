use super::*;

const WRITTEN: &str = "# for the climbing game\n[[group]]\nname = \"Celeste\"\nwindow = { exe = \"Celeste.exe\" }\nrules = [\n  { on = \"MouseLeft\", send = \"J\" }, # jump\n]\n";

#[test]
fn the_editor_changes_rules_and_keeps_the_comments() {
    let mut editor = editor(RemapState::On, Some(WRITTEN));

    editor.click("input-remaps.group.0.add-rule");
    editor.run();
    assert!(
        editor.shown("input-remaps.picker"),
        "a new remap asks for what is pressed"
    );
    all_keys(&mut editor);
    editor.click("input-remaps.picker.key.f20");
    editor.run();
    assert!(!editor.shown("input-remaps.picker"));
    pick_from_all_keys(&mut editor, "input-remaps.group.0.rule.1.key", "k");

    let text = stored(&editor);
    assert!(
        text.starts_with("# for the climbing game\n"),
        "the comments stay: {text}"
    );
    assert_eq!(text.matches("# jump").count(), 1, "{text}");
    assert!(text.contains(r#"{ on = "F20", send = "K" }"#), "{text}");

    editor.click("input-remaps.group.0.rule.1.remove");
    editor.run();
    editor.click("input-remaps.group.0.rule.0");
    editor.run();
    editor.click("input-remaps.group.0.rule.0.remove");
    editor.run();
    let text = stored(&editor);
    assert!(
        !text.contains("MouseLeft") && !text.contains("F20"),
        "{text}"
    );
}
