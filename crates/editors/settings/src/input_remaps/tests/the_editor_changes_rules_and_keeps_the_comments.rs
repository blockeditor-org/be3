use super::*;

const WRITTEN: &str = "# for the climbing game\n[[group]]\nname = \"Celeste\"\nwindow = { exe = \"Celeste.exe\" }\nrules = [\n  { on = \"MouseLeft\", send = \"J\" }, # jump\n]\n";

#[test]
fn the_editor_changes_rules_and_keeps_the_comments() {
    let mut editor = editor(RemapState::On, Some(WRITTEN));

    editor.click("input-remaps.group.0.add-rule");
    editor.run();
    editor.click("input-remaps.group.0.rule.1.key");
    editor.run();
    editor.key(
        block_editor_beui::beui::Key::A,
        block_editor_beui::beui::Modifiers::CTRL,
    );
    editor.type_text("K");
    editor.run();

    let text = stored(&editor);
    assert!(
        text.starts_with("# for the climbing game\n"),
        "the comments stay: {text}"
    );
    assert!(text.contains("# jump"), "{text}");
    assert!(text.contains(r#"{ on = "Space", send = "K" }"#), "{text}");
    editor.snapshot("the_editor_changes_rules_and_keeps_the_comments");

    editor.click("input-remaps.group.0.rule.0.remove");
    editor.run();
    assert!(!stored(&editor).contains("MouseLeft"));
}
