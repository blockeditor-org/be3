use super::*;

const TAP: &str = "input-settings.06cb:0001.tap-to-click";
const TAP_RESET: &str = "input-settings.06cb:0001.tap-to-click.reset";

#[test]
fn a_pointer_follows_its_device_default_until_it_is_set() {
    let mut editor = editor(vec![touchpad()]);
    let tap = |editor: &BeuiTest<InputSettingsApp>| {
        content(editor)
            .root()
            .pointer(&touchpad_identity())
            .tap_to_click
    };
    assert_eq!(tap(&editor), None);

    editor.click(TAP_RESET);
    editor.run();
    assert_eq!(
        tap(&editor),
        None,
        "the reset does nothing while the default is used"
    );

    editor.click(TAP);
    editor.run();
    assert_eq!(
        tap(&editor),
        Some(false),
        "the switch starts at the device's default, on"
    );

    editor.click(TAP);
    editor.run();
    assert_eq!(
        tap(&editor),
        Some(true),
        "a value equal to the default stays set"
    );

    editor.click(TAP);
    editor.run();
    editor.click(TAP_RESET);
    editor.run();
    assert_eq!(tap(&editor), None);
    assert!(content(&editor).root().pointers.is_empty());
    editor.snapshot("a_pointer_follows_its_device_default_until_it_is_set");
}
