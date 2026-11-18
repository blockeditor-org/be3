use super::*;

#[test]
fn picking_what_is_pressed_from_all_keys_writes_its_name() {
    let mut editor = editor(RemapState::On, Some(CAPS_TO_ESCAPE));

    editor.click("input-remaps.group.0.rule.0");
    editor.click("input-remaps.group.0.rule.0.on");
    all_keys(&mut editor);
    assert!(editor.shown("input-remaps.picker.key.f13"));
    editor.snapshot("the_key_picker_lists_every_key_by_kind");

    editor.click("input-remaps.picker.search");
    editor.type_text("right c");
    assert!(!editor.shown("input-remaps.picker.key.f13"));
    editor.click("input-remaps.picker.key.rightctrl");

    assert!(!editor.shown("input-remaps.picker"));
    let text = stored(&editor);
    assert!(
        text.contains(r#"{ on = "RightCtrl", send = "Escape" }"#),
        "the TOML gets the key's own name, not the label shown: {text}"
    );
}
