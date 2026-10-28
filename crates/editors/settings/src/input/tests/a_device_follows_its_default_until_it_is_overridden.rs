use super::*;

const TAP: &str = "input.pointer.tap-to-click";

#[test]
fn a_device_follows_its_default_until_it_is_overridden() {
    let mut editor = editor(vec![touchpad()]);
    choose_pointer(&mut editor, 1);
    let tap = |editor: &BeuiTest<InputSettingsApp>| {
        content(editor)
            .root()
            .device_pointer(&touchpad_identity())
            .tap_to_click
    };

    assert_eq!(
        tap(&editor),
        None,
        "nothing is set while the default is used"
    );

    editor.click(TAP);
    assert_eq!(
        tap(&editor),
        Some(false),
        "the switch starts at the device's default, on"
    );

    editor.click(TAP);
    assert_eq!(
        tap(&editor),
        Some(true),
        "a value equal to the default stays set"
    );

    reset(&mut editor, TAP);
    assert_eq!(tap(&editor), None);
    assert!(content(&editor).root().pointers.is_empty());
    assert_eq!(content(&editor).root().every_pointer.tap_to_click, None);
}
