use super::*;

#[test]
fn a_held_modifier_and_a_tap_held_for_a_preset_write_the_rule() {
    let mut editor = editor(RemapState::On, Some(CAPS_TO_ESCAPE));

    editor.click("input-remaps.group.0.rule.0");
    editor.run();
    editor.click("input-remaps.group.0.rule.0.held.ctrl");
    editor.run();
    editor.click("input-remaps.group.0.rule.0.held.shift");
    editor.run();
    assert!(
        stored(&editor).contains(r#"on = "Shift+Ctrl+CapsLock""#),
        "{}",
        stored(&editor)
    );
    editor.click("input-remaps.group.0.rule.0.held.ctrl");
    editor.run();
    assert!(
        stored(&editor).contains(r#"on = "Shift+CapsLock""#),
        "{}",
        stored(&editor)
    );

    choose_does(&mut editor, "input-remaps.group.0.rule.0", 1);
    editor.click("input-remaps.group.0.rule.0.hold.50");
    editor.run();
    let text = stored(&editor);
    assert!(
        text.contains(r#"{ on = "Shift+CapsLock", tap = "Escape", hold_ms = 50 }"#),
        "{text}"
    );
    editor.snapshot("an_open_remap_shows_its_editor");

    editor.click("input-remaps.group.0.rule.0.hold.0");
    editor.run();
    assert!(!stored(&editor).contains("hold_ms"), "{}", stored(&editor));
}
