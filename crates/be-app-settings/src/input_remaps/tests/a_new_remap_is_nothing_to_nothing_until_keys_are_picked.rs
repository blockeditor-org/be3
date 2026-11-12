use super::*;

#[test]
fn a_new_remap_is_nothing_to_nothing_until_keys_are_picked() {
    let mut editor = editor(RemapState::On, Some(CAPS_TO_ESCAPE));

    editor.click("input-remaps.group.0.add-rule");
    editor.run();
    assert!(editor.shown("input-remaps.picker"));
    editor.snapshot("a_new_remaps_picker_is_a_card_pointing_at_add_remap");

    editor.key(Key::Escape, Modifiers::NONE);
    editor.run();
    assert!(!editor.shown("input-remaps.picker"));
    let text = stored(&editor);
    assert!(
        text.contains(r#"{ on = "", send = "" }"#),
        "no key is chosen for you: {text}"
    );
    let groups = be_lib_remap::text::parse(&text).expect("an unfinished remap is stored");
    assert_eq!(groups[0].rules.len(), 1, "and does nothing: {text}");
    editor.snapshot("a_new_remap_is_nothing_to_nothing");

    pick_from_all_keys(&mut editor, "input-remaps.group.0.rule.1.on", "insert");
    editor.click("input-remaps.group.0.rule.1.key");
    editor.run();
    editor.snapshot("the_key_sent_can_be_nothing");
    editor.click("input-remaps.picker.nothing");
    editor.run();
    assert!(!editor.shown("input-remaps.picker"));
    let text = stored(&editor);
    assert!(text.contains(r#"{ on = "Insert", send = "" }"#), "{text}");
    let groups = be_lib_remap::text::parse(&text).expect("a key sent as nothing reads");
    assert_eq!(groups[0].rules[1].action, be_lib_remap::Action::NOTHING);

    editor.click("input-remaps.group.0.rule.1.on");
    editor.run();
    assert!(
        !editor.shown("input-remaps.picker.nothing"),
        "what is pressed is always something"
    );
}
