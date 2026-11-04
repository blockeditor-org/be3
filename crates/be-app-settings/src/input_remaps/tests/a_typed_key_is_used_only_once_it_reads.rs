use super::*;

#[test]
fn a_typed_key_is_used_only_once_it_reads() {
    let mut editor = editor(RemapState::On, Some(CAPS_TO_ESCAPE));

    editor.click("input-remaps.group.0.rule.0");
    editor.run();
    editor.click("input-remaps.group.0.rule.0.key");
    editor.run();
    editor.click("input-remaps.picker.typing");
    editor.run();
    editor.click("input-remaps.picker.typed");
    editor.run();
    editor.type_text("Spcae");
    editor.run();
    editor.click("input-remaps.picker.use");
    editor.run();
    assert!(
        editor.shown("input-remaps.picker"),
        "a name nothing reads is not used"
    );
    assert_eq!(stored(&editor), CAPS_TO_ESCAPE);

    editor.click("input-remaps.picker.typed");
    editor.run();
    editor.key(Key::A, Modifiers::CTRL);
    editor.type_text("Key 0x9e");
    editor.run();
    editor.click("input-remaps.picker.use");
    editor.run();
    assert!(!editor.shown("input-remaps.picker"));
    let text = stored(&editor);
    assert!(
        text.contains(r#"send = "Key:0x9e""#),
        "a code is written as the TOML reads it: {text}"
    );
    assert!(be_host_remap::text::parse(&text).is_ok(), "{text}");
}
