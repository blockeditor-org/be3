use super::*;

#[test]
fn in_a_windowed_desktop_the_keyboard_and_pasting_are_set_but_not_the_pointers() {
    let host = EditorHost::default();
    host.set_watched::<InputReachWatch>(&InputReach {
        keyboard: true,
        middle_click_paste: true,
    });
    let mut editor = open(host);
    editor.set_size(be_sdk_beui::beui::Vec2::new(800.0, 900.0));
    editor.run();
    for shown in [
        "input.layout",
        "input.variant",
        "input.options",
        "input.repeat-delay",
        "input.repeat-rate",
        "input.middle-click-paste",
    ] {
        assert!(editor.shown(shown), "{shown} takes effect in programs");
    }
    assert!(
        !editor.shown("input.pointer"),
        "a window's pointers are the system's around it"
    );
    assert!(!editor.shown("input.remaps"));

    for (field, text) in [
        ("input.layout", "us,de"),
        ("input.variant", "dvorak"),
        ("input.options", "caps:escape"),
    ] {
        editor.click(field);
        editor.type_text(text);
    }
    editor.click("input.repeat-delay");
    editor.key(Key::End, Modifiers::NONE);
    editor.click("input.repeat-rate");
    editor.key(Key::Home, Modifiers::NONE);
    editor.click("input.middle-click-paste");

    let set = content(&editor).root().clone();
    assert_eq!(set.keyboard_layout.as_deref(), Some("us,de"));
    assert_eq!(set.keyboard_variant.as_deref(), Some("dvorak"));
    assert_eq!(set.keyboard_options.as_deref(), Some("caps:escape"));
    assert_eq!(
        set.repeat_delay(),
        be_block_input::input_settings::MAX_REPEAT_DELAY
    );
    assert_eq!(
        set.repeat_rate(),
        be_block_input::input_settings::MIN_REPEAT_RATE
    );
    assert_eq!(set.middle_click_paste, Some(true));
    editor.snapshot("in_a_windowed_desktop_the_keyboard_and_pasting_are_set_but_not_the_pointers");

    for row in [
        "input.layout",
        "input.variant",
        "input.options",
        "input.repeat-delay",
        "input.repeat-rate",
        "input.middle-click-paste",
    ] {
        reset(&mut editor, row);
    }
    assert_eq!(content(&editor).root(), Default::default());
}
